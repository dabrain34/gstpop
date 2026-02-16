// main.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use gpop::gst::PipelineEvent;

#[cfg(target_os = "linux")]
use gpop::dbus::{run_dbus_event_forwarder, DbusServer};
use gpop::gst::{create_event_channel, PipelineManager};
use gpop::websocket::WebSocketServer;

#[derive(Parser, Debug)]
#[command(name = "gpop-rs")]
#[command(author = "Stéphane Cerveau")]
#[command(version)]
#[command(about = "GStreamer Prince of Parser - Pipeline management daemon")]
struct Args {
    /// WebSocket port
    #[arg(short = 'P', long, default_value_t = gpop::websocket::DEFAULT_WEBSOCKET_PORT)]
    port: u16,

    /// Bind address for WebSocket server
    #[arg(short, long, default_value = gpop::websocket::DEFAULT_BIND_ADDRESS)]
    bind: String,

    /// Initial pipeline(s) to create
    #[arg(short = 'p', long = "pipeline")]
    pipelines: Vec<String>,

    /// Playback mode: auto-play all pipelines and exit when all reach EOS
    #[arg(short = 'x', long)]
    playback_mode: bool,

    /// Disable DBus interface (Linux only)
    #[cfg(target_os = "linux")]
    #[arg(long)]
    no_dbus: bool,

    /// Disable WebSocket interface
    #[arg(long)]
    no_websocket: bool,

    /// API key for WebSocket authentication (optional)
    #[arg(long, env = "GPOP_API_KEY")]
    api_key: Option<String>,

    /// Allowed origins for WebSocket connections (optional, can be specified multiple times)
    /// If not specified, all origins are allowed. Use for CSRF protection in browser contexts.
    #[arg(long = "allowed-origin")]
    allowed_origins: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("gpop=info".parse().unwrap())
                .add_directive("gpop_rs=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    // Validate --playback-mode requires --pipeline
    if args.playback_mode && args.pipelines.is_empty() {
        error!("--playback-mode requires at least one --pipeline (-p) argument");
        std::process::exit(1);
    }

    // Validate that at least one interface is enabled
    #[cfg(target_os = "linux")]
    if args.no_dbus && args.no_websocket {
        error!("At least one interface (DBus or WebSocket) must be enabled");
        std::process::exit(1);
    }

    // Initialize GStreamer
    gstreamer::init()?;
    info!("GStreamer initialized");

    // Create event channel (receivers are created via event_tx.subscribe())
    let (event_tx, _) = create_event_channel();

    // Create pipeline manager
    let manager = Arc::new(PipelineManager::new(event_tx.clone()));

    // Create initial pipelines
    let mut initial_pipeline_ids: Vec<String> = Vec::new();
    for desc in &args.pipelines {
        match manager.add_pipeline(desc).await {
            Ok(id) => {
                info!("Created initial pipeline '{}': {}", id, desc);
                initial_pipeline_ids.push(id);
            }
            Err(e) => error!("Failed to create initial pipeline '{}': {}", desc, e),
        }
    }

    // Subscribe early so no events are missed during auto-play
    let playback_event_rx = if args.playback_mode {
        Some(event_tx.subscribe())
    } else {
        None
    };

    // Auto-play pipelines in playback mode
    let playback_failed_ids = if args.playback_mode {
        if initial_pipeline_ids.is_empty() {
            error!("Playback mode: no pipelines were created successfully, exiting");
            std::process::exit(1);
        }
        let failed = manager.play_all(&initial_pipeline_ids).await;
        info!(
            "Playback mode: started {} pipeline(s)",
            initial_pipeline_ids.len() - failed.len()
        );
        failed
    } else {
        HashSet::new()
    };

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
                    std::process::exit(1);
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
        let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
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

    // Exit codes matching GStreamer convention (gst-launch MR !10088)
    const EXIT_CODE_ERROR: i32 = 1;
    const EXIT_CODE_UNSUPPORTED: i32 = 69; // EX_UNAVAILABLE

    // Set up playback mode EOS tracking
    let playback_done: Option<tokio::sync::oneshot::Receiver<i32>> = if args.playback_mode {
        let mut event_rx = playback_event_rx.expect("playback_event_rx set when playback_mode");
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<i32>();

        let mut pending: HashSet<String> = initial_pipeline_ids
            .iter()
            .filter(|id| !playback_failed_ids.contains(*id))
            .cloned()
            .collect();

        let had_error_initially = !playback_failed_ids.is_empty();
        let started_count = pending.len();
        let tracker_manager = Arc::clone(&manager);

        tokio::spawn(async move {
            let mut had_error = had_error_initially;
            let mut had_unsupported = false;

            // If all pipelines already failed to play, signal immediately
            if pending.is_empty() {
                info!("Playback mode: all pipelines failed to start");
                let _ = done_tx.send(EXIT_CODE_ERROR);
                return;
            }

            loop {
                match event_rx.recv().await {
                    Ok(event) => match &event {
                        PipelineEvent::Eos { pipeline_id } => {
                            if pending.remove(pipeline_id) {
                                info!(
                                    "Playback mode: pipeline '{}' reached EOS ({}/{} remaining)",
                                    pipeline_id,
                                    pending.len(),
                                    started_count
                                );
                            }
                        }
                        PipelineEvent::Error {
                            pipeline_id,
                            message,
                        } => {
                            if pending.remove(pipeline_id) {
                                had_error = true;
                                warn!(
                                    "Playback mode: pipeline '{}' errored: {} ({}/{} remaining)",
                                    pipeline_id,
                                    message,
                                    pending.len(),
                                    started_count
                                );
                            }
                        }
                        PipelineEvent::Unsupported {
                            pipeline_id,
                            message,
                        } => {
                            if pending.remove(pipeline_id) {
                                had_unsupported = true;
                                warn!(
                                    "Playback mode: pipeline '{}' unsupported: {} ({}/{} remaining)",
                                    pipeline_id,
                                    message,
                                    pending.len(),
                                    started_count
                                );
                            }
                        }
                        PipelineEvent::PipelineRemoved { pipeline_id } => {
                            if pending.remove(pipeline_id) {
                                had_error = true;
                                warn!(
                                    "Playback mode: tracked pipeline '{}' was removed externally ({}/{} remaining)",
                                    pipeline_id,
                                    pending.len(),
                                    started_count
                                );
                            }
                        }
                        _ => {}
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(
                            "Playback mode: event tracker lagged by {} messages, reconciling",
                            n
                        );
                        // Reconcile: remove pipelines that no longer exist in the manager
                        let gone: Vec<String> = {
                            let mut removed = Vec::new();
                            for id in &pending {
                                if tracker_manager.get_pipeline_info(id).await.is_err() {
                                    removed.push(id.clone());
                                }
                            }
                            removed
                        };
                        for id in gone {
                            pending.remove(&id);
                            had_error = true;
                            warn!(
                                "Playback mode: pipeline '{}' no longer exists after lag ({}/{} remaining)",
                                id,
                                pending.len(),
                                started_count
                            );
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        warn!("Playback mode: event channel closed before all pipelines finished");
                        had_error = true;
                        pending.clear();
                    }
                }

                if pending.is_empty() {
                    info!("Playback mode: all pipelines finished");
                    // Error takes priority over Unsupported
                    let code = if had_error {
                        EXIT_CODE_ERROR
                    } else if had_unsupported {
                        EXIT_CODE_UNSUPPORTED
                    } else {
                        0
                    };
                    let _ = done_tx.send(code);
                    return;
                }
            }
        });

        Some(done_rx)
    } else {
        None
    };

    // Wait for shutdown signal
    info!("gpop-rs started. Press Ctrl+C to stop.");

    let mut exit_code: i32 = 0;

    let playback_future = async {
        match playback_done {
            Some(rx) => rx.await,
            None => std::future::pending().await,
        }
    };

    // Register signal handlers before entering select! (registration is synchronous and fallible)
    #[cfg(unix)]
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    let shutdown_signal = async {
        #[cfg(unix)]
        {
            tokio::select! {
                _ = sigint.recv() => info!("Received SIGINT"),
                _ = sigterm.recv() => info!("Received SIGTERM"),
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen for Ctrl+C");
            info!("Received Ctrl+C");
        }
    };

    tokio::select! {
        _ = shutdown_signal => {}
        result = playback_future => {
            match result {
                Ok(code) => {
                    exit_code = code;
                    if exit_code == 0 {
                        info!("Playback mode: all pipelines completed successfully");
                    } else {
                        warn!("Playback mode: exiting with code {}", exit_code);
                    }
                }
                Err(_) => {
                    error!("Playback mode: tracker task dropped unexpectedly");
                    exit_code = 1;
                }
            }
        }
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

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}
