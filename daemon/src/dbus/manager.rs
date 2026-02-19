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

#[interface(name = "org.gpop.Manager")]
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
        ObjectPath::from_static_str("/org/gpop/Manager").unwrap()
    }
}
