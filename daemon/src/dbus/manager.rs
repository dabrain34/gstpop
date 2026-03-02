// manager.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use zbus::object_server::SignalEmitter;
use zbus::{interface, zvariant::ObjectPath};

use crate::gst::PipelineManager;

pub struct ManagerInterface {
    pub manager: Arc<PipelineManager>,
}

#[interface(name = "org.gstpop.Manager")]
impl ManagerInterface {
    async fn add_pipeline(&self, description: &str) -> zbus::fdo::Result<String> {
        self.manager
            .add_pipeline(description)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn remove_pipeline(&self, id: &str) -> zbus::fdo::Result<()> {
        self.manager
            .remove_pipeline(id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn get_pipeline_desc(&self, id: &str) -> zbus::fdo::Result<String> {
        self.manager
            .get_pipeline_description(id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn get_elements(&self, detail: &str) -> zbus::fdo::Result<String> {
        let detail_level = detail
            .parse::<crate::gst::registry::DetailLevel>()
            .map_err(zbus::fdo::Error::Failed)?;
        // Registry iteration is CPU-bound; run off the async runtime
        let elements =
            tokio::task::spawn_blocking(move || crate::gst::registry::get_elements(detail_level))
                .await
                .map_err(|e| zbus::fdo::Error::Failed(format!("Registry query failed: {}", e)))?;
        serde_json::to_string(&elements).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn discover_uri(&self, uri: &str, timeout: u32) -> zbus::fdo::Result<String> {
        let uri = uri.to_string();
        let timeout_opt = if timeout == 0 { None } else { Some(timeout) };

        let result = tokio::task::spawn_blocking(move || {
            crate::gst::discoverer::discover_uri(&uri, timeout_opt)
        })
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("Discovery task failed: {}", e)))?
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        serde_json::to_string(&result).map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    async fn play_uri(
        &self,
        uri: &str,
        video_sink: &str,
        audio_sink: &str,
        use_playbin2: bool,
    ) -> zbus::fdo::Result<String> {
        let vs = if video_sink.is_empty() {
            None
        } else {
            Some(video_sink)
        };
        let a_s = if audio_sink.is_empty() {
            None
        } else {
            Some(audio_sink)
        };

        let description =
            crate::gst::discoverer::build_playbin_description(uri, vs, a_s, use_playbin2)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let pipeline_id = self
            .manager
            .add_pipeline(&description)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        if let Err(e) = self.manager.play(&pipeline_id).await {
            // Clean up the pipeline we just created so it doesn't consume a slot
            let _ = self.manager.remove_pipeline(&pipeline_id).await;
            return Err(zbus::fdo::Error::Failed(e.to_string()));
        }

        Ok(pipeline_id)
    }

    async fn update_pipeline(&self, id: &str, description: &str) -> zbus::fdo::Result<()> {
        self.manager
            .update_pipeline(id, description)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    #[zbus(property)]
    async fn pipelines(&self) -> u32 {
        self.manager.pipeline_count().await as u32
    }

    #[zbus(property)]
    async fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    #[zbus(property, name = "GStreamerVersion")]
    async fn gstreamer_version(&self) -> String {
        gstreamer::version_string().to_string()
    }

    #[zbus(signal)]
    async fn pipeline_added(
        emitter: &SignalEmitter<'_>,
        id: &str,
        description: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn pipeline_removed(emitter: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;
}

impl ManagerInterface {
    pub fn new(manager: Arc<PipelineManager>) -> Self {
        Self { manager }
    }

    pub fn object_path() -> ObjectPath<'static> {
        ObjectPath::from_static_str("/org/gstpop/Manager").unwrap()
    }
}
