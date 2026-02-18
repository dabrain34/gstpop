// playback.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;
use std::sync::Arc;

use tracing::{info, warn};

use crate::gst::{EventReceiver, PipelineEvent, PipelineManager};

/// Exit codes matching GStreamer convention (gst-launch MR !10088)
pub const EXIT_CODE_ERROR: i32 = 1;
pub const EXIT_CODE_UNSUPPORTED: i32 = 69; // EX_UNAVAILABLE

/// Tracks playback progress for a set of pipelines and signals when all are done.
pub struct PlaybackTracker {
    pending: HashSet<String>,
    had_error: bool,
    had_unsupported: bool,
    started_count: usize,
    manager: Arc<PipelineManager>,
}

impl PlaybackTracker {
    /// Create a new tracker for the given pipeline IDs.
    /// `failed_ids` are pipelines that already failed to start.
    pub fn new(
        ids: &[String],
        failed_ids: &HashSet<String>,
        manager: Arc<PipelineManager>,
    ) -> Self {
        let pending: HashSet<String> = ids
            .iter()
            .filter(|id| !failed_ids.contains(*id))
            .cloned()
            .collect();

        Self {
            started_count: pending.len(),
            had_error: !failed_ids.is_empty(),
            had_unsupported: false,
            pending,
            manager,
        }
    }

    /// Run the playback tracker, consuming events until all pipelines are done.
    /// Returns the exit code (0 = success, 1 = error, 69 = unsupported media).
    pub async fn run(mut self, mut event_rx: EventReceiver) -> i32 {
        // If all pipelines already failed to play, return immediately
        if self.pending.is_empty() {
            info!("Playback mode: all pipelines failed to start");
            return EXIT_CODE_ERROR;
        }

        loop {
            match event_rx.recv().await {
                Ok(event) => match &event {
                    PipelineEvent::Eos { pipeline_id } => {
                        if self.pending.remove(pipeline_id) {
                            info!(
                                "Playback mode: pipeline '{}' reached EOS ({}/{} remaining)",
                                pipeline_id,
                                self.pending.len(),
                                self.started_count
                            );
                        }
                    }
                    PipelineEvent::Error {
                        pipeline_id,
                        message,
                    } => {
                        if self.pending.remove(pipeline_id) {
                            self.had_error = true;
                            warn!(
                                "Playback mode: pipeline '{}' errored: {} ({}/{} remaining)",
                                pipeline_id,
                                message,
                                self.pending.len(),
                                self.started_count
                            );
                        }
                    }
                    PipelineEvent::Unsupported {
                        pipeline_id,
                        message,
                    } => {
                        if self.pending.remove(pipeline_id) {
                            self.had_unsupported = true;
                            warn!(
                                "Playback mode: pipeline '{}' unsupported: {} ({}/{} remaining)",
                                pipeline_id,
                                message,
                                self.pending.len(),
                                self.started_count
                            );
                        }
                    }
                    PipelineEvent::PipelineRemoved { pipeline_id } => {
                        if self.pending.remove(pipeline_id) {
                            self.had_error = true;
                            warn!(
                                "Playback mode: tracked pipeline '{}' was removed externally ({}/{} remaining)",
                                pipeline_id,
                                self.pending.len(),
                                self.started_count
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
                        for id in &self.pending {
                            if self.manager.get_pipeline_info(id).await.is_err() {
                                removed.push(id.clone());
                            }
                        }
                        removed
                    };
                    for id in gone {
                        self.pending.remove(&id);
                        self.had_error = true;
                        warn!(
                            "Playback mode: pipeline '{}' no longer exists after lag ({}/{} remaining)",
                            id,
                            self.pending.len(),
                            self.started_count
                        );
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    warn!("Playback mode: event channel closed before all pipelines finished");
                    self.had_error = true;
                    self.pending.clear();
                }
            }

            if self.pending.is_empty() {
                info!("Playback mode: all pipelines finished");
                // Error takes priority over Unsupported
                return if self.had_error {
                    EXIT_CODE_ERROR
                } else if self.had_unsupported {
                    EXIT_CODE_UNSUPPORTED
                } else {
                    0
                };
            }
        }
    }
}
