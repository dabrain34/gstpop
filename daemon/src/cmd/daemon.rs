// cmd/daemon.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::net::SocketAddr;
use std::sync::Arc;

use clap::Args;
use tracing::{error, info, warn};

#[cfg(target_os = "linux")]
use gpop::dbus::{run_dbus_event_forwarder, DbusServer};
use gpop::gst::{create_event_channel, PipelineManager};
use gpop::websocket::WebSocketServer;

/// Start the daemon with WebSocket and DBus interfaces
#[derive(Args, Debug)]
pub struct DaemonArgs {
    /// WebSocket port
    #[arg(short = 'P', long, default_value_t = gpop::websocket::DEFAULT_WEBSOCKET_PORT)]
    pub port: u16,

    /// Bind address for WebSocket server
    #[arg(short, long, default_value = gpop::websocket::DEFAULT_BIND_ADDRESS)]
    pub bind: String,

    /// Initial pipeline(s) to create
    #[arg(short = 'p', long = "pipeline")]
    pub pipelines: Vec<String>,

    /// Disable DBus interface (Linux only)
    #[cfg(target_os = "linux")]
    #[arg(long)]
    pub no_dbus: bool,

    /// Disable WebSocket interface
    #[arg(long)]
    pub no_websocket: bool,

    /// API key for WebSocket authentication (optional)
    #[arg(long, env = "GPOP_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    /// Allowed origins for WebSocket connections (optional, can be specified multiple times)
    /// If not specified, all origins are allowed. Use for CSRF protection in browser contexts.
    #[arg(long = "allowed-origin")]
    pub allowed_origins: Vec<String>,
}

pub async fn run(args: DaemonArgs) -> i32 {
    // Validate that at least one interface is enabled
    #[cfg(target_os = "linux")]
    if args.no_dbus && args.no_websocket {
        error!("At least one interface (DBus or WebSocket) must be enabled");
        return 1;
    }

    // On non-Linux, WebSocket is the only interface — disabling it is invalid
    #[cfg(not(target_os = "linux"))]
    if args.no_websocket {
        error!("WebSocket is the only available interface on this platform");
        return 1;
    }

    // Create event channel (receivers are created via event_tx.subscribe())
    let (event_tx, _) = create_event_channel();

    // Create pipeline manager
    let manager = Arc::new(PipelineManager::new(event_tx.clone()));

    // Create initial pipelines
    for desc in &args.pipelines {
        match manager.add_pipeline(desc).await {
            Ok(id) => info!("Created initial pipeline '{}': {}", id, desc),
            Err(e) => error!("Failed to create initial pipeline '{}': {}", desc, e),
        }
    }

    // Start DBus server (Linux only)
    #[cfg(target_os = "linux")]
    let dbus_server = if !args.no_dbus {
        match DbusServer::new(Arc::clone(&manager)).await {
            Ok(server) => {
                let server = Arc::new(server);

                // Start DBus event forwarder
                let dbus_server_clone = Arc::clone(&server);
                let dbus_event_rx = event_tx.subscribe();
                tokio::spawn(async move {
                    run_dbus_event_forwarder(dbus_server_clone, dbus_event_rx).await;
                });

                Some(server)
            }
            Err(e) => {
                error!("Failed to start DBus server: {}", e);
                if args.no_websocket {
                    return 1;
                }
                None
            }
        }
    } else {
        info!("DBus interface disabled");
        None
    };

    // Start WebSocket server
    let ws_handle = if !args.no_websocket {
        let addr: SocketAddr = match format!("{}:{}", args.bind, args.port).parse() {
            Ok(addr) => addr,
            Err(e) => {
                error!("Invalid address: {}", e);
                return 1;
            }
        };
        let allowed_origins = if args.allowed_origins.is_empty() {
            None
        } else {
            Some(args.allowed_origins.clone())
        };
        let ws_server = WebSocketServer::new(
            addr,
            Arc::clone(&manager),
            args.api_key.clone(),
            allowed_origins.clone(),
        );
        let ws_event_rx = event_tx.subscribe();

        // Safety check: warn or refuse non-loopback binding without API key
        let is_loopback = addr.ip().is_loopback();
        if !is_loopback && args.api_key.is_none() {
            warn!(
                "Binding to non-loopback address {} without --api-key is insecure. \
                 Set GPOP_API_KEY or use --api-key to require authentication.",
                addr
            );
        }
        if !is_loopback && args.api_key.is_some() {
            warn!(
                "API key is transmitted in plaintext over ws://{}. \
                 Consider using a TLS-terminating reverse proxy for production.",
                addr
            );
        }

        if args.api_key.is_some() {
            info!("WebSocket API key authentication enabled");
        }
        if let Some(ref origins) = allowed_origins {
            info!("WebSocket origin validation enabled for: {:?}", origins);
        }

        Some(tokio::spawn(async move {
            if let Err(e) = ws_server.run(ws_event_rx).await {
                error!("WebSocket server error: {}", e);
            }
        }))
    } else {
        info!("WebSocket interface disabled");
        None
    };

    // Wait for shutdown signal
    info!("gpop daemon started. Press Ctrl+C to stop.");

    if let Err(e) = gpop::signal::wait_for_shutdown().await {
        error!("{}", e);
        return 1;
    }

    // Graceful shutdown
    info!("Shutting down...");

    // Stop pipelines
    manager.shutdown().await;

    // Cancel WebSocket server
    if let Some(handle) = ws_handle {
        handle.abort();
    }

    // DBus connection will be dropped automatically (Linux only)
    #[cfg(target_os = "linux")]
    drop(dbus_server);

    info!("Shutdown complete");

    0
}
