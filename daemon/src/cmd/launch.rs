// cmd/launch.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;

use clap::Args;
use tracing::{error, info, warn};

use gstpop::gst::{create_event_channel, PipelineManager};
use gstpop::playback::PlaybackTracker;
use gstpop::server::ServerHandle;

/// Launch pipelines and exit when all finish
#[derive(Args, Debug)]
#[command(trailing_var_arg = true)]
pub struct LaunchArgs {
    /// Pipeline description(s) to launch
    #[arg(short = 'p', long = "pipeline")]
    pub pipelines: Vec<String>,

    /// Pipeline description (positional, e.g. videotestsrc ! autovideosink)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub pipeline: Vec<String>,

    #[command(flatten)]
    pub server: super::common::ServerArgs,
}

pub async fn run(args: LaunchArgs) -> i32 {
    // Merge positional pipeline with -p flag pipelines
    let mut all_pipelines = Vec::new();
    if !args.pipeline.is_empty() {
        all_pipelines.push(args.pipeline.join(" "));
    }
    all_pipelines.extend(args.pipelines.iter().cloned());

    if all_pipelines.is_empty() {
        error!("No pipeline description provided");
        return 1;
    }

    let (event_tx, _) = create_event_channel();
    let manager = Arc::new(PipelineManager::new(event_tx.clone()));

    // Start servers (non-fatal — playback continues even if servers fail)
    let servers = ServerHandle::start(args.server.into_config(), Arc::clone(&manager), &event_tx)
        .await
        .ok();

    // Create pipelines
    let mut ids = Vec::new();
    let mut creation_failures = 0usize;
    for desc in &all_pipelines {
        match manager.add_pipeline(desc).await {
            Ok(id) => {
                info!("Created pipeline '{}': {}", id, desc);
                ids.push(id);
            }
            Err(e) => {
                error!("Failed to create pipeline '{}': {}", desc, e);
                creation_failures += 1;
            }
        }
    }

    if ids.is_empty() {
        error!("No pipelines were created successfully");
        if let Some(s) = servers {
            s.shutdown();
        }
        return 1;
    }

    // Subscribe before playing so no events are missed
    let event_rx = event_tx.subscribe();

    // Play all pipelines
    let failed = manager.play_all(&ids).await;
    info!("Started {} pipeline(s)", ids.len() - failed.len());

    let tracker = PlaybackTracker::new(&ids, &failed, Arc::clone(&manager));

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<i32>();
    tokio::spawn(async move {
        let code = tracker.run(event_rx).await;
        let _ = done_tx.send(code);
    });

    let exit_code = tokio::select! {
        result = gstpop::signal::wait_for_shutdown() => {
            if let Err(e) = result {
                error!("{}", e);
            }
            warn!("Interrupted, shutting down");
            1
        }
        result = done_rx => {
            match result {
                Ok(code) => {
                    let code = if code == 0 && creation_failures > 0 { 1 } else { code };
                    if code == 0 {
                        info!("All pipelines completed successfully");
                    } else {
                        warn!("Exiting with code {}", code);
                    }
                    code
                }
                Err(_) => {
                    error!("Playback tracker dropped unexpectedly");
                    1
                }
            }
        }
    };

    manager.shutdown().await;
    if let Some(s) = servers {
        s.shutdown();
    }
    exit_code
}
