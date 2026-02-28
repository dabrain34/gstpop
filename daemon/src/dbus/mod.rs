// mod.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

pub mod manager;
pub mod pipeline;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use zbus::connection::Builder;
use zbus::Connection;

use crate::error::Result;
use crate::gst::{EventReceiver, PipelineEvent, PipelineManager};

use self::manager::ManagerInterface;
use self::pipeline::PipelineInterface;

pub const DBUS_SERVICE_NAME: &str = "org.gstpop";

pub struct DbusServer {
    connection: Connection,
    manager: Arc<PipelineManager>,
    /// Maps pipeline_id -> DBus object index
    pipeline_indices: RwLock<HashMap<String, u32>>,
    /// Next index to use for a new pipeline
    next_index: AtomicU32,
}

impl DbusServer {
    pub async fn new(manager: Arc<PipelineManager>) -> Result<Self> {
        let manager_interface = ManagerInterface::new(Arc::clone(&manager));

        let connection = Builder::session()?
            .name(DBUS_SERVICE_NAME)?
            .serve_at(ManagerInterface::object_path(), manager_interface)?
            .build()
            .await?;

        info!(
            "DBus server started on session bus as '{}'",
            DBUS_SERVICE_NAME
        );

        Ok(Self {
            connection,
            manager,
            pipeline_indices: RwLock::new(HashMap::new()),
            next_index: AtomicU32::new(0),
        })
    }

    pub async fn register_pipeline(&self, pipeline_id: &str) -> Result<()> {
        let index = self.next_index.fetch_add(1, Ordering::Relaxed);

        let pipeline_interface =
            PipelineInterface::new(Arc::clone(&self.manager), pipeline_id.to_string());

        self.connection
            .object_server()
            .at(PipelineInterface::object_path(index), pipeline_interface)
            .await?;

        // Store the mapping
        {
            let mut indices = self.pipeline_indices.write().await;
            indices.insert(pipeline_id.to_string(), index);
        }

        info!(
            "Registered pipeline '{}' at DBus path /org/gstpop/Pipeline{}",
            pipeline_id, index
        );

        Ok(())
    }

    pub async fn unregister_pipeline(&self, pipeline_id: &str) -> Result<()> {
        // Look up the index for this pipeline
        let index = {
            let mut indices = self.pipeline_indices.write().await;
            indices.remove(pipeline_id)
        };

        if let Some(index) = index {
            let path = PipelineInterface::object_path(index);
            self.connection
                .object_server()
                .remove::<PipelineInterface, _>(&path)
                .await?;

            info!(
                "Unregistered pipeline '{}' from DBus path {:?}",
                pipeline_id, path
            );
        } else {
            warn!(
                "Attempted to unregister unknown pipeline '{}' from DBus",
                pipeline_id
            );
        }

        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

pub async fn run_dbus_event_forwarder(dbus_server: Arc<DbusServer>, mut event_rx: EventReceiver) {
    loop {
        match event_rx.recv().await {
            Ok(event) => match event {
                PipelineEvent::PipelineAdded {
                    pipeline_id,
                    description: _,
                } => {
                    if let Err(e) = dbus_server.register_pipeline(&pipeline_id).await {
                        error!("Failed to register pipeline on DBus: {}", e);
                    }
                }
                PipelineEvent::PipelineRemoved { pipeline_id } => {
                    if let Err(e) = dbus_server.unregister_pipeline(&pipeline_id).await {
                        error!("Failed to unregister pipeline from DBus: {}", e);
                    }
                }
                _ => {
                    // State changes, errors, EOS are handled via DBus signals
                    // when properties are queried
                }
            },
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                warn!("DBus event forwarder lagged by {} messages", n);
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                info!("Event channel closed, stopping DBus event forwarder");
                break;
            }
        }
    }
}
