// pipeline.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use zbus::object_server::SignalEmitter;
use zbus::{interface, zvariant::ObjectPath};

use crate::gst::{PipelineManager, PipelineState};

pub struct PipelineInterface {
    pub manager: Arc<PipelineManager>,
    pub pipeline_id: String,
}

#[interface(name = "org.gpop.Pipeline")]
impl PipelineInterface {
    async fn set_state(&self, state: &str) -> zbus::fdo::Result<bool> {
        let state: PipelineState = state
            .parse()
            .map_err(|e: String| zbus::fdo::Error::Failed(e))?;

        self.manager
            .set_state(&self.pipeline_id, state)
            .await
            .map(|_| true)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn play(&self) -> zbus::fdo::Result<bool> {
        self.manager
            .play(&self.pipeline_id)
            .await
            .map(|_| true)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn pause(&self) -> zbus::fdo::Result<bool> {
        self.manager
            .pause(&self.pipeline_id)
            .await
            .map(|_| true)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn stop(&self) -> zbus::fdo::Result<bool> {
        self.manager
            .stop(&self.pipeline_id)
            .await
            .map(|_| true)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get the DOT graph representation of the pipeline.
    /// The `details` parameter controls the level of detail:
    /// - "media": Show media type
    /// - "caps": Show caps details
    /// - "non-default": Show non-default parameters
    /// - "states": Show element states
    /// - "all" or empty: Show all details
    async fn get_dot(&self, details: &str) -> zbus::fdo::Result<String> {
        let details = if details.is_empty() {
            None
        } else {
            Some(details)
        };
        self.manager
            .get_dot(&self.pipeline_id, details)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Get the current position and duration of the pipeline in nanoseconds.
    /// Returns a tuple of (position, duration) where either may be -1 if not available.
    async fn get_position(&self) -> zbus::fdo::Result<(i64, i64)> {
        let (position, duration) = self
            .manager
            .get_position(&self.pipeline_id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        // Convert Option<u64> to i64, using -1 for None
        // Use try_from to avoid silent overflow for values above i64::MAX
        let pos = position
            .map(|p| i64::try_from(p).unwrap_or(i64::MAX))
            .unwrap_or(-1);
        let dur = duration
            .map(|d| i64::try_from(d).unwrap_or(i64::MAX))
            .unwrap_or(-1);
        Ok((pos, dur))
    }

    /// Update the pipeline with a new description.
    /// This stops the current pipeline and creates a new one with the same ID.
    async fn update(&self, description: &str) -> zbus::fdo::Result<bool> {
        self.manager
            .update_pipeline(&self.pipeline_id, description)
            .await
            .map(|_| true)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    #[zbus(property)]
    async fn id(&self) -> &str {
        &self.pipeline_id
    }

    #[zbus(property)]
    async fn description(&self) -> zbus::fdo::Result<String> {
        self.manager
            .get_pipeline_description(&self.pipeline_id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    #[zbus(property, name = "State")]
    async fn current_state(&self) -> zbus::fdo::Result<String> {
        let info = self
            .manager
            .get_pipeline_info(&self.pipeline_id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(info.state.to_string())
    }

    #[zbus(property)]
    async fn streaming(&self) -> zbus::fdo::Result<bool> {
        let info = self
            .manager
            .get_pipeline_info(&self.pipeline_id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(info.streaming)
    }

    #[zbus(signal, name = "StateChanged")]
    async fn emit_state_changed(
        emitter: &SignalEmitter<'_>,
        old_state: &str,
        new_state: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn error(emitter: &SignalEmitter<'_>, message: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn eos(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

impl PipelineInterface {
    pub fn new(manager: Arc<PipelineManager>, pipeline_id: String) -> Self {
        Self {
            manager,
            pipeline_id,
        }
    }

    pub fn object_path(index: u32) -> ObjectPath<'static> {
        ObjectPath::try_from(format!("/org/gpop/Pipeline{}", index))
            .expect("u32 index always produces valid D-Bus object path")
    }
}
