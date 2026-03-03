// cmd/daemon.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;

use clap::Args;
use tracing::{error, info};

use gstpop::gst::{create_event_channel, PipelineManager};
use gstpop::server::ServerHandle;

/// Start the daemon with WebSocket and DBus interfaces
#[derive(Args, Debug)]
pub struct DaemonArgs {
    /// Initial pipeline(s) to create
    #[arg(short = 'p', long = "pipeline")]
    pub pipelines: Vec<String>,

    #[command(flatten)]
    pub server: super::common::ServerArgs,
}

pub async fn run(args: DaemonArgs) -> i32 {
    let config = args.server.into_config();

    // Validate that at least one interface is enabled
    #[cfg(target_os = "linux")]
    if config.no_dbus && config.no_websocket {
        error!("At least one interface (DBus or WebSocket) must be enabled");
        return 1;
    }

    // On non-Linux, WebSocket is the only interface — disabling it is invalid
    #[cfg(not(target_os = "linux"))]
    if config.no_websocket {
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

    // Start servers (fatal for daemon — it needs at least one interface)
    let servers = match ServerHandle::start(config, Arc::clone(&manager), &event_tx).await {
        Ok(s) => s,
        Err(()) => {
            error!("Failed to start any server interface");
            return 1;
        }
    };

    // Wait for shutdown signal
    info!("gst-pop daemon started. Press Ctrl+C to stop.");

    if let Err(e) = gstpop::signal::wait_for_shutdown().await {
        error!("{}", e);
        return 1;
    }

    // Graceful shutdown
    info!("Shutting down...");
    manager.shutdown().await;
    servers.shutdown();
    info!("Shutdown complete");

    0
}
