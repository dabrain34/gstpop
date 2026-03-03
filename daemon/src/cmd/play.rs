// cmd/play.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;

use clap::Args;
use tracing::{error, info, warn};

use gstpop::gst::discoverer::build_playbin_description;
use gstpop::gst::{create_event_channel, PipelineManager};
use gstpop::playback::PlaybackTracker;
use gstpop::server::ServerHandle;

/// Play a media URI using playbin
#[derive(Args, Debug)]
pub struct PlayArgs {
    /// URI or file path to play
    pub uri: String,

    /// Video sink element (e.g., autovideosink, fakesink)
    #[arg(long)]
    pub video_sink: Option<String>,

    /// Audio sink element (e.g., autoaudiosink, fakesink)
    #[arg(long)]
    pub audio_sink: Option<String>,

    /// Use legacy playbin instead of playbin3
    #[arg(long)]
    pub playbin2: bool,

    #[command(flatten)]
    pub server: super::common::ServerArgs,
}

pub async fn run(args: PlayArgs) -> i32 {
    let description = match build_playbin_description(
        &args.uri,
        args.video_sink.as_deref(),
        args.audio_sink.as_deref(),
        args.playbin2,
    ) {
        Ok(d) => d,
        Err(e) => {
            error!("Invalid URI: {}", e);
            return 1;
        }
    };

    let (event_tx, _) = create_event_channel();
    let manager = Arc::new(PipelineManager::new(event_tx.clone()));

    // Start servers (non-fatal — playback continues even if servers fail)
    let servers = ServerHandle::start(args.server.into_config(), Arc::clone(&manager), &event_tx)
        .await
        .ok();

    let id = match manager.add_pipeline(&description).await {
        Ok(id) => {
            info!("Created pipeline '{}': {}", id, description);
            id
        }
        Err(e) => {
            error!("Failed to create pipeline: {}", e);
            if let Some(s) = servers {
                s.shutdown();
            }
            return 1;
        }
    };

    // Subscribe before playing so no events are missed
    let event_rx = event_tx.subscribe();

    let failed = manager.play_all(std::slice::from_ref(&id)).await;
    if !failed.is_empty() {
        error!("Failed to start playback for URI: {}", args.uri);
        manager.shutdown().await;
        if let Some(s) = servers {
            s.shutdown();
        }
        return 1;
    }
    info!("Playing {}", args.uri);

    let tracker = PlaybackTracker::new(&[id], &failed, Arc::clone(&manager));

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
                    if code == 0 {
                        info!("Playback completed successfully");
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
