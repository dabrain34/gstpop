// server.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::task::JoinHandle;
use tracing::{error, info, warn};

#[cfg(target_os = "linux")]
use crate::dbus::{run_dbus_event_forwarder, DbusServer};
use crate::gst::{EventSender, PipelineManager};
use crate::websocket::WebSocketServer;

/// Configuration for WebSocket and DBus server interfaces.
pub struct ServerConfig {
    pub bind: String,
    pub port: u16,
    pub no_websocket: bool,
    pub no_dbus: bool,
    pub api_key: Option<String>,
    pub allowed_origins: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: crate::websocket::DEFAULT_BIND_ADDRESS.to_string(),
            port: crate::websocket::DEFAULT_WEBSOCKET_PORT,
            no_websocket: false,
            #[cfg(target_os = "linux")]
            no_dbus: false,
            #[cfg(not(target_os = "linux"))]
            no_dbus: true,
            api_key: None,
            allowed_origins: Vec::new(),
        }
    }
}

/// Handle to running WebSocket and DBus server tasks.
///
/// Call `shutdown()` to stop the servers gracefully.
pub struct ServerHandle {
    ws_handle: Option<JoinHandle<()>>,
    #[cfg(target_os = "linux")]
    _dbus_server: Option<Arc<DbusServer>>,
}

impl ServerHandle {
    /// Start WebSocket and/or DBus servers.
    ///
    /// Returns `Ok(ServerHandle)` on success, `Err(())` if all requested
    /// servers failed to start.
    pub async fn start(
        config: ServerConfig,
        manager: Arc<PipelineManager>,
        event_tx: &EventSender,
    ) -> Result<Self, ()> {
        // Start DBus server (Linux only)
        #[cfg(target_os = "linux")]
        let dbus_server = if !config.no_dbus {
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
                    None
                }
            }
        } else {
            None
        };

        // Start WebSocket server
        let ws_handle = if !config.no_websocket {
            let addr: SocketAddr = match format!("{}:{}", config.bind, config.port).parse() {
                Ok(addr) => addr,
                Err(e) => {
                    error!("Invalid address: {}", e);
                    return Err(());
                }
            };
            let allowed_origins = if config.allowed_origins.is_empty() {
                None
            } else {
                Some(config.allowed_origins.clone())
            };
            let ws_server = WebSocketServer::new(
                addr,
                Arc::clone(&manager),
                config.api_key.clone(),
                allowed_origins.clone(),
            );
            let ws_event_rx = event_tx.subscribe();

            // Safety check: warn about non-loopback binding without API key
            let is_loopback = addr.ip().is_loopback();
            if !is_loopback && config.api_key.is_none() {
                warn!(
                    "Binding to non-loopback address {} without --api-key is insecure. \
                     Set GSTPOP_API_KEY or use --api-key to require authentication.",
                    addr
                );
            }
            if !is_loopback && config.api_key.is_some() {
                warn!(
                    "API key is transmitted in plaintext over ws://{}. \
                     Consider using a TLS-terminating reverse proxy for production.",
                    addr
                );
            }

            if config.api_key.is_some() {
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
            None
        };

        Ok(Self {
            ws_handle,
            #[cfg(target_os = "linux")]
            _dbus_server: dbus_server,
        })
    }

    /// Shut down all running servers.
    pub fn shutdown(self) {
        if let Some(handle) = self.ws_handle {
            handle.abort();
        }
        // DBus connection is dropped automatically
    }
}
