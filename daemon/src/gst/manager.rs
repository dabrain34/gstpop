// manager.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};

use super::{MAX_PIPELINES, SHUTDOWN_GRACE_PERIOD_MS};
use crate::error::{GpopError, Result};
use crate::gst::event::{EventSender, PipelineEvent, PipelineState};
use crate::gst::pipeline::Pipeline;

pub struct PipelineInfo {
    pub id: String,
    pub description: String,
    pub state: PipelineState,
    pub streaming: bool,
}

pub struct PipelineManager {
    pipelines: RwLock<HashMap<String, Arc<Mutex<Pipeline>>>>,
    event_tx: EventSender,
    next_id: AtomicU64,
}

impl PipelineManager {
    pub fn new(event_tx: EventSender) -> Self {
        Self {
            pipelines: RwLock::new(HashMap::new()),
            event_tx,
            next_id: AtomicU64::new(0),
        }
    }

    pub async fn add_pipeline(&self, description: &str) -> Result<String> {
        // Check pipeline limit before creating
        {
            let pipelines = self.pipelines.read().await;
            if pipelines.len() >= MAX_PIPELINES {
                return Err(GpopError::InvalidPipeline(format!(
                    "Maximum number of pipelines ({}) reached",
                    MAX_PIPELINES
                )));
            }
        }

        // Use Relaxed ordering - we only need uniqueness, not synchronization
        // Using u64 makes overflow practically impossible (would take millions of years
        // at 1 billion pipelines per second)
        let id_num = self.next_id.fetch_add(1, Ordering::Relaxed);

        let id = id_num.to_string();

        let pipeline = Pipeline::new(id.clone(), description)?;
        let pipeline = Arc::new(Mutex::new(pipeline));

        // Extract bus watch parameters synchronously to avoid race conditions
        let (bus, shutdown_flag, pipeline_obj) = {
            let p = pipeline.lock().await;
            let bus = p
                .bus()
                .ok_or_else(|| GpopError::InvalidPipeline("Pipeline has no bus".to_string()))?;
            (bus, p.shutdown_flag(), p.pipeline_object())
        };

        // Start bus watcher and get the task handle
        let bus_task = Pipeline::start_bus_watch(
            bus,
            id.clone(),
            self.event_tx.clone(),
            shutdown_flag,
            pipeline_obj,
        );

        // Store the task handle synchronously
        {
            let mut p = pipeline.lock().await;
            p.set_bus_task(bus_task);
        }

        {
            let mut pipelines = self.pipelines.write().await;
            // Re-check limit under write lock to prevent TOCTOU race
            if pipelines.len() >= MAX_PIPELINES {
                return Err(GpopError::InvalidPipeline(format!(
                    "Maximum number of pipelines ({}) reached",
                    MAX_PIPELINES
                )));
            }
            pipelines.insert(id.clone(), pipeline);
        }

        info!("Added pipeline '{}': {}", id, description);

        if self
            .event_tx
            .send(PipelineEvent::PipelineAdded {
                pipeline_id: id.clone(),
                description: description.to_string(),
            })
            .is_err()
        {
            warn!("Failed to send PipelineAdded event: no receivers");
        }

        Ok(id)
    }

    pub async fn remove_pipeline(&self, id: &str) -> Result<()> {
        let pipeline = {
            let mut pipelines = self.pipelines.write().await;
            pipelines.remove(id)
        };
        // Write lock released before stopping the pipeline

        if let Some(pipeline) = pipeline {
            {
                let p = pipeline.lock().await;
                p.stop()?;
            }

            info!("Removed pipeline '{}'", id);

            if self
                .event_tx
                .send(PipelineEvent::PipelineRemoved {
                    pipeline_id: id.to_string(),
                })
                .is_err()
            {
                warn!("Failed to send PipelineRemoved event: no receivers");
            }

            Ok(())
        } else {
            Err(GpopError::PipelineNotFound(id.to_string()))
        }
    }

    pub async fn get_pipeline(&self, id: &str) -> Result<Arc<Mutex<Pipeline>>> {
        let pipelines = self.pipelines.read().await;
        pipelines
            .get(id)
            .cloned()
            .ok_or_else(|| GpopError::PipelineNotFound(id.to_string()))
    }

    pub async fn get_pipeline_info(&self, id: &str) -> Result<PipelineInfo> {
        let pipeline = self.get_pipeline(id).await?;
        let p = pipeline.lock().await;

        Ok(PipelineInfo {
            id: p.id().to_string(),
            description: p.description().to_string(),
            state: p.state(),
            streaming: p.is_streaming(),
        })
    }

    pub async fn get_pipeline_description(&self, id: &str) -> Result<String> {
        let pipeline = self.get_pipeline(id).await?;
        let p = pipeline.lock().await;
        Ok(p.description().to_string())
    }

    pub async fn list_pipelines(&self) -> Vec<PipelineInfo> {
        // Collect pipeline references while holding the read lock briefly
        let pipeline_refs: Vec<Arc<Mutex<Pipeline>>> = {
            let pipelines = self.pipelines.read().await;
            pipelines.values().cloned().collect()
        };
        // Read lock is now released

        // Now iterate over pipelines without holding the outer lock
        let mut infos = Vec::with_capacity(pipeline_refs.len());
        for pipeline in pipeline_refs {
            let p = pipeline.lock().await;
            infos.push(PipelineInfo {
                id: p.id().to_string(),
                description: p.description().to_string(),
                state: p.state(),
                streaming: p.is_streaming(),
            });
        }

        infos
    }

    pub async fn pipeline_count(&self) -> usize {
        let pipelines = self.pipelines.read().await;
        pipelines.len()
    }

    pub async fn set_state(&self, id: &str, state: PipelineState) -> Result<()> {
        let pipeline = self.get_pipeline(id).await?;
        let p = pipeline.lock().await;
        p.set_state(state)
    }

    pub async fn play(&self, id: &str) -> Result<()> {
        self.set_state(id, PipelineState::Playing).await
    }

    /// Play multiple pipelines concurrently, returning the set of IDs that failed to start.
    pub async fn play_all(&self, ids: &[String]) -> std::collections::HashSet<String> {
        let futures: Vec<_> = ids
            .iter()
            .map(|id| async move {
                match self.play(id).await {
                    Ok(()) => None,
                    Err(e) => {
                        warn!("Failed to play pipeline '{}': {}", id, e);
                        Some(id.clone())
                    }
                }
            })
            .collect();

        futures_util::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    pub async fn pause(&self, id: &str) -> Result<()> {
        self.set_state(id, PipelineState::Paused).await
    }

    pub async fn stop(&self, id: &str) -> Result<()> {
        self.set_state(id, PipelineState::Null).await
    }

    pub async fn get_dot(&self, id: &str, details: Option<&str>) -> Result<String> {
        let pipeline = self.get_pipeline(id).await?;
        let p = pipeline.lock().await;
        Ok(p.get_dot(details))
    }

    /// Get the current position and duration of a pipeline in nanoseconds.
    pub async fn get_position(&self, id: &str) -> Result<(Option<u64>, Option<u64>)> {
        let pipeline = self.get_pipeline(id).await?;
        let p = pipeline.lock().await;
        Ok(p.get_position())
    }

    /// Update an existing pipeline with a new description.
    /// This stops the old pipeline, removes it, and creates a new one with the same ID.
    pub async fn update_pipeline(&self, id: &str, description: &str) -> Result<()> {
        // Create the new pipeline first (validates the description before acquiring locks)
        // This allows early failure without holding any locks
        let new_pipeline = Pipeline::new(id.to_string(), description)?;
        let new_pipeline = Arc::new(Mutex::new(new_pipeline));

        // Extract bus watch parameters for the new pipeline
        let (bus, shutdown_flag, pipeline_obj) = {
            let p = new_pipeline.lock().await;
            let bus = p
                .bus()
                .ok_or_else(|| GpopError::InvalidPipeline("Pipeline has no bus".to_string()))?;
            (bus, p.shutdown_flag(), p.pipeline_object())
        };

        // Acquire write lock and perform atomic check-and-swap
        // This prevents TOCTOU race conditions
        let mut pipelines = self.pipelines.write().await;

        // Verify the pipeline exists while holding the lock
        if !pipelines.contains_key(id) {
            // Drop the new pipeline (will clean up resources)
            drop(new_pipeline);
            return Err(GpopError::PipelineNotFound(id.to_string()));
        }

        // Start bus watcher for the new pipeline (after confirming old pipeline exists)
        let bus_task = Pipeline::start_bus_watch(
            bus,
            id.to_string(),
            self.event_tx.clone(),
            shutdown_flag,
            pipeline_obj,
        );

        // Store the task handle
        {
            let mut p = new_pipeline.lock().await;
            p.set_bus_task(bus_task);
        }

        // Swap: insert new pipeline, extract old one
        let old_pipeline = pipelines.remove(id);
        pipelines.insert(id.to_string(), new_pipeline);

        // Release the write lock before stopping old pipeline
        drop(pipelines);

        // Stop old pipeline outside the lock
        if let Some(old_pipeline) = old_pipeline {
            let p = old_pipeline.lock().await;
            let _ = p.stop();
        }

        info!("Updated pipeline '{}': {}", id, description);

        // Send event to notify clients
        if self
            .event_tx
            .send(PipelineEvent::PipelineUpdated {
                pipeline_id: id.to_string(),
                description: description.to_string(),
            })
            .is_err()
        {
            warn!("Failed to send PipelineUpdated event: no receivers");
        }

        Ok(())
    }

    pub async fn shutdown(&self) {
        let pipelines_to_stop: Vec<_> = {
            let mut pipelines = self.pipelines.write().await;
            pipelines.drain().collect()
        };

        // Signal all pipelines to shutdown first
        for (_, pipeline) in &pipelines_to_stop {
            let p = pipeline.lock().await;
            p.signal_shutdown();
        }

        // Single grace period for all bus watchers
        tokio::time::sleep(tokio::time::Duration::from_millis(SHUTDOWN_GRACE_PERIOD_MS)).await;

        // Now stop all pipelines
        for (id, pipeline) in pipelines_to_stop {
            let p = pipeline.lock().await;
            let _ = p.stop();
            info!("Stopped pipeline '{}' during shutdown", id);
        }
    }
}
