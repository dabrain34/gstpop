// manager.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use tracing::{debug, error};

use crate::gst::{PipelineManager, PipelineState};

use super::pipeline::*;
use super::protocol::*;
use super::DEFAULT_PIPELINE_ID;

/// Safely convert a serializable value to a JSON Value.
/// Returns an internal error response if serialization fails (should never happen for well-typed structs).
fn to_json_value<T: serde::Serialize>(id: serde_json::Value, value: &T) -> Response {
    match serde_json::to_value(value) {
        Ok(v) => Response::success(id, v),
        Err(e) => {
            error!("JSON serialization failed: {}", e);
            Response::error(
                id,
                error_codes::INTERNAL_ERROR,
                "Internal serialization error".to_string(),
            )
        }
    }
}

/// WebSocket interface for managing pipelines.
/// This is the WebSocket equivalent of the DBus `ManagerInterface`.
pub struct ManagerInterface {
    manager: Arc<PipelineManager>,
}

impl ManagerInterface {
    pub fn new(manager: Arc<PipelineManager>) -> Self {
        Self { manager }
    }

    pub async fn handle(&self, request: Request) -> Response {
        debug!("Handling request: {} (id: {})", request.method, request.id);

        match request.method.as_str() {
            "list_pipelines" => self.list_pipelines(request.id).await,
            "create_pipeline" => self.create_pipeline(request).await,
            "remove_pipeline" => self.remove_pipeline(request).await,
            "get_pipeline_info" => self.get_pipeline_info(request).await,
            "set_state" => self.set_state(request).await,
            "play" => self.play(request).await,
            "pause" => self.pause(request).await,
            "stop" => self.stop(request).await,
            "get_position" => self.get_position(request).await,
            "update_pipeline" => self.update_pipeline(request).await,
            "get_version" => self.get_version(request.id),
            "get_info" => self.get_info(request.id),
            "get_pipeline_count" => self.get_pipeline_count(request.id).await,
            "get_elements" => self.get_elements(request).await,
            "discover_uri" => self.discover_uri(request).await,
            // snapshot is handled separately in server.rs
            _ => Response::method_not_found(request.id, &request.method),
        }
    }

    /// Get the daemon version
    fn get_version(&self, id: serde_json::Value) -> Response {
        let result = VersionResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        to_json_value(id, &result)
    }

    /// Get daemon and GStreamer version info
    fn get_info(&self, id: serde_json::Value) -> Response {
        let result = InfoResult {
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            gstreamer_version: gstreamer::version_string().to_string(),
            jsonrpc_version: JSONRPC_VERSION.to_string(),
        };
        to_json_value(id, &result)
    }

    /// Get the number of managed pipelines
    async fn get_pipeline_count(&self, id: serde_json::Value) -> Response {
        let count = self.manager.pipeline_count().await;
        let result = PipelineCountResult { count };
        to_json_value(id, &result)
    }

    async fn get_elements(&self, request: Request) -> Response {
        let params: GetElementsParams =
            serde_json::from_value(request.params).unwrap_or(GetElementsParams { detail: None });

        let detail_str = params.detail.as_deref().unwrap_or("none");
        let detail = match detail_str.parse::<crate::gst::registry::DetailLevel>() {
            Ok(d) => d,
            Err(e) => return Response::invalid_params(request.id, e),
        };

        // Registry iteration is CPU-bound; run off the async runtime
        let elements =
            match tokio::task::spawn_blocking(move || crate::gst::registry::get_elements(detail))
                .await
            {
                Ok(elems) => elems,
                Err(e) => {
                    error!("get_elements task failed: {}", e);
                    return Response::error(
                        request.id,
                        error_codes::INTERNAL_ERROR,
                        "Registry query failed".to_string(),
                    );
                }
            };

        let result = GetElementsResult { elements };
        to_json_value(request.id, &result)
    }

    async fn discover_uri(&self, request: Request) -> Response {
        let params: DiscoverUriParams = match serde_json::from_value(request.params) {
            Ok(p) => p,
            Err(e) => {
                return Response::invalid_params(request.id, format!("Invalid params: {}", e))
            }
        };

        let uri = params.uri;
        let timeout = params.timeout;

        match tokio::task::spawn_blocking(move || {
            crate::gst::discoverer::discover_uri(&uri, timeout)
        })
        .await
        {
            Ok(Ok(info)) => {
                let result = DiscoverUriResult { info };
                to_json_value(request.id, &result)
            }
            Ok(Err(e)) => Response::from_gpop_error(request.id, &e),
            Err(e) => Response::error(
                request.id,
                error_codes::INTERNAL_ERROR,
                format!("Discovery task failed: {}", e),
            ),
        }
    }

    async fn list_pipelines(&self, id: serde_json::Value) -> Response {
        let infos = self.manager.list_pipelines().await;
        let mut pipelines: Vec<PipelineInfoResult> =
            infos.into_iter().map(PipelineInfoResult::from).collect();

        // Sort by ID for deterministic ordering
        pipelines.sort_by(|a, b| {
            // Try numeric comparison first, fall back to string comparison
            match (a.id.parse::<u64>(), b.id.parse::<u64>()) {
                (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
                _ => a.id.cmp(&b.id),
            }
        });

        let result = ListPipelinesResult { pipelines };
        to_json_value(id, &result)
    }

    async fn create_pipeline(&self, request: Request) -> Response {
        let params: CreatePipelineParams = match serde_json::from_value(request.params) {
            Ok(p) => p,
            Err(e) => {
                return Response::invalid_params(request.id, format!("Invalid params: {}", e))
            }
        };

        match self.manager.add_pipeline(&params.description).await {
            Ok(pipeline_id) => {
                let result = PipelineCreatedResult { pipeline_id };
                to_json_value(request.id, &result)
            }
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }

    async fn remove_pipeline(&self, request: Request) -> Response {
        let params: PipelineIdParams = match serde_json::from_value(request.params) {
            Ok(p) => p,
            Err(e) => {
                return Response::invalid_params(request.id, format!("Invalid params: {}", e))
            }
        };

        match self.manager.remove_pipeline(&params.pipeline_id).await {
            Ok(()) => Response::success(request.id, serde_json::json!({})),
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }

    async fn get_pipeline_info(&self, request: Request) -> Response {
        let params: PipelineIdParams = match serde_json::from_value(request.params) {
            Ok(p) => p,
            Err(e) => {
                return Response::invalid_params(request.id, format!("Invalid params: {}", e))
            }
        };

        match self.manager.get_pipeline_info(&params.pipeline_id).await {
            Ok(info) => {
                let result = PipelineInfoResult::from(info);
                to_json_value(request.id, &result)
            }
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }

    async fn set_state(&self, request: Request) -> Response {
        let params: SetStateParams = match serde_json::from_value(request.params) {
            Ok(p) => p,
            Err(e) => {
                return Response::invalid_params(request.id, format!("Invalid params: {}", e))
            }
        };

        let state: PipelineState = match params.state.parse() {
            Ok(s) => s,
            Err(e) => return Response::invalid_params(request.id, e),
        };

        match self.manager.set_state(&params.pipeline_id, state).await {
            Ok(()) => {
                let result = SuccessResult { success: true };
                to_json_value(request.id, &result)
            }
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }

    async fn play(&self, request: Request) -> Response {
        let params: OptionalPipelineIdParams =
            serde_json::from_value(request.params).unwrap_or_default();

        let pipeline_id = params
            .pipeline_id
            .unwrap_or_else(|| DEFAULT_PIPELINE_ID.to_string());

        match self.manager.play(&pipeline_id).await {
            Ok(()) => {
                let result = SuccessResult { success: true };
                to_json_value(request.id, &result)
            }
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }

    async fn pause(&self, request: Request) -> Response {
        let params: OptionalPipelineIdParams =
            serde_json::from_value(request.params).unwrap_or_default();

        let pipeline_id = params
            .pipeline_id
            .unwrap_or_else(|| DEFAULT_PIPELINE_ID.to_string());

        match self.manager.pause(&pipeline_id).await {
            Ok(()) => {
                let result = SuccessResult { success: true };
                to_json_value(request.id, &result)
            }
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }

    async fn stop(&self, request: Request) -> Response {
        let params: OptionalPipelineIdParams =
            serde_json::from_value(request.params).unwrap_or_default();

        let pipeline_id = params
            .pipeline_id
            .unwrap_or_else(|| DEFAULT_PIPELINE_ID.to_string());

        match self.manager.stop(&pipeline_id).await {
            Ok(()) => {
                let result = SuccessResult { success: true };
                to_json_value(request.id, &result)
            }
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }

    pub async fn snapshot(
        &self,
        params: SnapshotParams,
    ) -> Result<SnapshotResult, crate::error::GpopError> {
        let pipeline_id = params
            .pipeline_id
            .unwrap_or_else(|| DEFAULT_PIPELINE_ID.to_string());

        let dot = self
            .manager
            .get_dot(&pipeline_id, params.details.as_deref())
            .await?;

        Ok(SnapshotResult {
            response_type: "SnapshotResponse".to_string(),
            pipelines: vec![PipelineSnapshot {
                id: pipeline_id,
                dot,
            }],
        })
    }

    async fn get_position(&self, request: Request) -> Response {
        let params: OptionalPipelineIdParams =
            serde_json::from_value(request.params).unwrap_or_default();

        let pipeline_id = params
            .pipeline_id
            .unwrap_or_else(|| DEFAULT_PIPELINE_ID.to_string());

        match self.manager.get_position(&pipeline_id).await {
            Ok((position_ns, duration_ns)) => {
                let progress = match (position_ns, duration_ns) {
                    (Some(pos), Some(dur)) if dur > 0 => {
                        // Clamp progress to 0.0..=1.0 range
                        // (position can briefly exceed duration during seeks)
                        Some((pos as f64 / dur as f64).clamp(0.0, 1.0))
                    }
                    _ => None,
                };

                let result = PositionResult {
                    position_ns,
                    duration_ns,
                    progress,
                };
                to_json_value(request.id, &result)
            }
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }

    async fn update_pipeline(&self, request: Request) -> Response {
        let params: UpdatePipelineParams = match serde_json::from_value(request.params) {
            Ok(p) => p,
            Err(e) => {
                return Response::invalid_params(request.id, format!("Invalid params: {}", e))
            }
        };

        match self
            .manager
            .update_pipeline(&params.pipeline_id, &params.description)
            .await
        {
            Ok(()) => {
                let result = SuccessResult { success: true };
                to_json_value(request.id, &result)
            }
            Err(e) => Response::from_gpop_error(request.id, &e),
        }
    }
}
