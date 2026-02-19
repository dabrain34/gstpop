// pipeline.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use gstreamer::prelude::*;
use gstreamer::{self as gst, DebugGraphDetails};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::error::{GpopError, Result};
use crate::gst::event::{EventSender, PipelineEvent, PipelineState};

/// Maximum length for pipeline descriptions to prevent memory exhaustion
pub const MAX_PIPELINE_DESCRIPTION_LENGTH: usize = 64 * 1024; // 64KB

/// Check if a GStreamer error indicates unsupported media (missing codec, format, etc.)
/// Returns Some with a descriptive message if it's a media error, None otherwise.
pub fn is_media_not_supported_error(error: &gst::glib::Error) -> Option<String> {
    let message = error.message();
    let msg_lower = message.to_lowercase();

    // Check for common patterns in GStreamer error messages indicating media issues
    // These patterns cover missing codecs, unsupported formats, and hardware limitations
    let media_patterns = [
        "no suitable",
        "missing plugin",
        "missing element",
        "codec not found",
        "could not determine type",
        "unhandled",
        "not supported",
        "unsupported",
        "no decoder",
        "no encoder",
        "no demuxer",
        "no muxer",
        "format not supported",
        "caps not supported",
        "not negotiated",
        "stream type not supported",
    ];

    for pattern in &media_patterns {
        if msg_lower.contains(pattern) {
            return Some(message.to_string());
        }
    }

    None
}

/// Timeout for state changes in seconds
pub const STATE_CHANGE_TIMEOUT_SECS: u64 = 30;

pub struct Pipeline {
    id: String,
    description: String,
    pipeline: gst::Pipeline,
    bus_task: Option<tokio::task::JoinHandle<()>>,
    /// Flag to signal the bus watcher to stop
    shutdown_flag: Arc<AtomicBool>,
}

impl Pipeline {
    pub fn new(id: String, description: &str) -> Result<Self> {
        // Validate description is not empty or whitespace-only
        let trimmed = description.trim();
        if trimmed.is_empty() {
            return Err(GpopError::InvalidPipeline(
                "Pipeline description cannot be empty".to_string(),
            ));
        }

        // Validate description length
        if description.len() > MAX_PIPELINE_DESCRIPTION_LENGTH {
            return Err(GpopError::InvalidPipeline(format!(
                "Pipeline description too long: {} bytes (max: {} bytes)",
                description.len(),
                MAX_PIPELINE_DESCRIPTION_LENGTH
            )));
        }

        // Note: gst::init() must be called once at startup in main.rs before creating pipelines.
        // We don't call it here to avoid masking initialization errors.

        let pipeline = gst::parse::launch(description)
            .map_err(|e| {
                // Check if this is a media-related error (missing codec, unsupported format, etc.)
                if let Some(msg) = is_media_not_supported_error(&e) {
                    GpopError::MediaNotSupported(msg)
                } else {
                    GpopError::InvalidPipeline(e.to_string())
                }
            })?
            .downcast::<gst::Pipeline>()
            .map_err(|_| GpopError::InvalidPipeline("Not a pipeline".to_string()))?;

        info!("Created pipeline '{}': {}", id, description);

        Ok(Self {
            id,
            description: description.to_string(),
            pipeline,
            bus_task: None,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Get a reference to the underlying GStreamer pipeline as a GObject.
    /// Used for bus message source comparison without requiring a mutex lock.
    pub fn pipeline_object(&self) -> gst::Object {
        self.pipeline.clone().upcast()
    }

    /// Start the bus watcher task for this pipeline.
    /// The bus, pipeline ID, event sender, shutdown flag, and pipeline object are extracted
    /// synchronously before spawning to avoid race conditions with pipeline destruction.
    pub fn start_bus_watch(
        bus: gst::Bus,
        id: String,
        event_tx: EventSender,
        shutdown_flag: Arc<AtomicBool>,
        pipeline_obj: gst::Object,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                // Check shutdown flag first (use Acquire to synchronize with Release store)
                if shutdown_flag.load(Ordering::Acquire) {
                    debug!("Bus watcher for pipeline '{}' received shutdown signal", id);
                    break;
                }

                // Clone for use in spawn_blocking (bus is Send + Sync)
                let bus_clone = bus.clone();
                let shutdown_clone = Arc::clone(&shutdown_flag);

                // Use spawn_blocking to avoid blocking the async runtime
                let msg = match tokio::task::spawn_blocking(move || {
                    // Check shutdown flag again inside blocking context
                    if shutdown_clone.load(Ordering::Acquire) {
                        return None;
                    }
                    let timeout = gst::ClockTime::from_mseconds(100);
                    bus_clone.timed_pop(timeout)
                })
                .await
                {
                    Ok(msg) => msg,
                    Err(e) => {
                        // spawn_blocking panicked or was cancelled - log and continue
                        warn!(
                            "Bus watcher spawn_blocking failed for pipeline '{}': {}",
                            id, e
                        );
                        continue;
                    }
                };

                if let Some(msg) = msg {
                    match msg.view() {
                        gst::MessageView::Error(err) => {
                            let gst_error = err.error();
                            // Log full debug info (including C source paths) server-side only
                            let debug_info = err.debug().unwrap_or_default();
                            let full_msg = format!("{}: {}", gst_error, debug_info);
                            // Only send the GError message to clients, not debug details
                            let client_msg = gst_error.to_string();

                            // Check if this is a media-related error
                            let event = if is_media_not_supported_error(&gst_error).is_some() {
                                warn!("Pipeline '{}' unsupported media: {}", id, full_msg);
                                PipelineEvent::Unsupported {
                                    pipeline_id: id.clone(),
                                    message: client_msg,
                                }
                            } else {
                                error!("Pipeline '{}' error: {}", id, full_msg);
                                PipelineEvent::Error {
                                    pipeline_id: id.clone(),
                                    message: client_msg,
                                }
                            };

                            if event_tx.send(event).is_err() {
                                warn!(
                                    "Failed to send error event for pipeline '{}': no receivers",
                                    id
                                );
                            }
                        }
                        gst::MessageView::Warning(warning) => {
                            warn!(
                                "Pipeline '{}' warning: {}",
                                id,
                                warning.debug().unwrap_or_default()
                            );
                        }
                        gst::MessageView::Eos(_) => {
                            info!("Pipeline '{}' reached end of stream", id);
                            if event_tx
                                .send(PipelineEvent::Eos {
                                    pipeline_id: id.clone(),
                                })
                                .is_err()
                            {
                                warn!(
                                    "Failed to send EOS event for pipeline '{}': no receivers",
                                    id
                                );
                            }
                        }
                        gst::MessageView::StateChanged(state_changed) => {
                            if let Some(src) = msg.src() {
                                if *src == pipeline_obj {
                                    let old = PipelineState::from(state_changed.old());
                                    let new = PipelineState::from(state_changed.current());
                                    debug!("Pipeline '{}' state changed: {} -> {}", id, old, new);
                                    if event_tx
                                        .send(PipelineEvent::StateChanged {
                                            pipeline_id: id.clone(),
                                            old_state: old,
                                            new_state: new,
                                        })
                                        .is_err()
                                    {
                                        warn!(
                                            "Failed to send state change event for pipeline '{}': no receivers",
                                            id
                                        );
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            debug!("Bus watcher for pipeline '{}' stopped", id);
        })
    }

    /// Get the GStreamer bus for this pipeline
    pub fn bus(&self) -> Option<gst::Bus> {
        self.pipeline.bus()
    }

    /// Set the bus task handle
    pub fn set_bus_task(&mut self, task: tokio::task::JoinHandle<()>) {
        self.bus_task = Some(task);
    }

    /// Get the shutdown flag
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown_flag)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn state(&self) -> PipelineState {
        // state() returns (Result<StateChangeSuccess, StateChangeError>, State, State)
        let (_result, current, _pending) = self.pipeline.state(gst::ClockTime::ZERO);
        PipelineState::from(current)
    }

    pub fn is_streaming(&self) -> bool {
        matches!(self.state(), PipelineState::Playing)
    }

    pub fn set_state(&self, state: PipelineState) -> Result<()> {
        let gst_state: gst::State = state.into();
        self.pipeline
            .set_state(gst_state)
            .map_err(|e| GpopError::StateChangeFailed(e.to_string()))?;

        // Wait for state change with timeout
        let timeout = gst::ClockTime::from_seconds(STATE_CHANGE_TIMEOUT_SECS);
        let (result, current, _pending) = self.pipeline.state(timeout);

        match result {
            Ok(success) => {
                match success {
                    gst::StateChangeSuccess::Success | gst::StateChangeSuccess::NoPreroll => {
                        info!("Pipeline '{}' state set to {}", self.id, state);
                        Ok(())
                    }
                    gst::StateChangeSuccess::Async => {
                        // State change is still in progress but was accepted
                        info!(
                            "Pipeline '{}' state change to {} in progress (current: {:?})",
                            self.id, state, current
                        );
                        Ok(())
                    }
                }
            }
            Err(_) => Err(GpopError::StateChangeFailed(format!(
                "Failed to change state to {} for pipeline '{}'",
                state, self.id
            ))),
        }
    }

    pub fn play(&self) -> Result<()> {
        self.set_state(PipelineState::Playing)
    }

    pub fn pause(&self) -> Result<()> {
        self.set_state(PipelineState::Paused)
    }

    pub fn stop(&self) -> Result<()> {
        self.set_state(PipelineState::Null)
    }

    pub fn get_dot(&self, details: Option<&str>) -> String {
        let detail_flags = match details {
            Some("media") => DebugGraphDetails::MEDIA_TYPE,
            Some("caps") => DebugGraphDetails::CAPS_DETAILS,
            Some("non-default") => DebugGraphDetails::NON_DEFAULT_PARAMS,
            Some("states") => DebugGraphDetails::STATES,
            Some("all") | None => DebugGraphDetails::all(),
            Some(_) => DebugGraphDetails::all(),
        };

        self.pipeline.debug_to_dot_data(detail_flags).to_string()
    }

    /// Get the current position and duration of the pipeline in nanoseconds.
    /// Returns (position_ns, duration_ns) where either value may be None if not available.
    pub fn get_position(&self) -> (Option<u64>, Option<u64>) {
        let position = self
            .pipeline
            .query_position::<gst::ClockTime>()
            .map(|p| p.nseconds());

        let duration = self
            .pipeline
            .query_duration::<gst::ClockTime>()
            .map(|d| d.nseconds());

        (position, duration)
    }

    /// Signal the bus watcher to stop
    pub fn signal_shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::Release);
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        debug!("Dropping pipeline '{}'", self.id);

        // Signal bus watcher to stop (use Release to synchronize with Acquire load)
        self.shutdown_flag.store(true, Ordering::Release);

        // Set pipeline to Null state
        let _ = self.pipeline.set_state(gst::State::Null);

        // Abort the bus task if it exists
        if let Some(task) = self.bus_task.take() {
            task.abort();
        }
    }
}
