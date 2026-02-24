// protocol.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

//! Generic WebSocket protocol types for JSON-RPC communication.
//!
//! This module contains the core Request/Response types and manager-level
//! operations. Pipeline-specific types are in the `pipeline` module.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 standard error codes
pub mod error_codes {
    /// Parse error - Invalid JSON was received
    pub const PARSE_ERROR: i32 = -32700;
    /// Invalid Request - The JSON sent is not a valid Request object
    pub const INVALID_REQUEST: i32 = -32600;
    /// Method not found - The method does not exist / is not available
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid params - Invalid method parameter(s)
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal error - Internal JSON-RPC error
    pub const INTERNAL_ERROR: i32 = -32603;

    // Server error codes (reserved for implementation-defined server errors)
    // Range: -32000 to -32099

    /// Pipeline not found
    pub const PIPELINE_NOT_FOUND: i32 = -32000;
    /// Pipeline creation failed
    pub const PIPELINE_CREATION_FAILED: i32 = -32001;
    /// State change failed
    pub const STATE_CHANGE_FAILED: i32 = -32002;
    /// GStreamer error
    pub const GSTREAMER_ERROR: i32 = -32003;
    /// Pipeline description too long
    pub const DESCRIPTION_TOO_LONG: i32 = -32004;
    /// Media not supported (missing codec, unsupported format, hardware limitation)
    pub const MEDIA_NOT_SUPPORTED: i32 = -32005;
    /// Discovery failed (timeout, URI not found, etc.)
    pub const DISCOVERY_FAILED: i32 = -32006;
}

/// JSON-RPC 2.0 version string
pub const JSONRPC_VERSION: &str = "2.0";

fn default_jsonrpc_version() -> String {
    JSONRPC_VERSION.to_string()
}

/// JSON-RPC 2.0 Request object.
///
/// Per the JSON-RPC 2.0 specification:
/// - `id` can be a String, Number, or Null; omitted for notifications
/// - `method` is required and specifies the method to invoke
/// - `jsonrpc` should be "2.0" (defaults if omitted for compatibility)
/// - `params` is optional
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    /// JSON-RPC version (should be "2.0")
    #[serde(default = "default_jsonrpc_version")]
    pub jsonrpc: String,
    /// Request identifier — String, Number, or Null per JSON-RPC 2.0.
    /// Defaults to Null if omitted (notification).
    #[serde(default)]
    pub id: Value,
    /// Method name to invoke - required per JSON-RPC 2.0 spec
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: &'static str,
    /// Echoed request id — String, Number, or Null per JSON-RPC 2.0
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorInfo {
    pub code: i32,
    pub message: String,
}

impl Response {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: None,
            error: Some(ErrorInfo { code, message }),
        }
    }

    /// Create a parse error response
    pub fn parse_error(id: Value, message: String) -> Self {
        Self::error(id, error_codes::PARSE_ERROR, message)
    }

    /// Create an invalid request error response (missing required fields)
    pub fn invalid_request(id: Value, message: String) -> Self {
        Self::error(id, error_codes::INVALID_REQUEST, message)
    }

    /// Create a method not found error response
    pub fn method_not_found(id: Value, method: &str) -> Self {
        Self::error(
            id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", method),
        )
    }

    /// Create an invalid params error response
    pub fn invalid_params(id: Value, message: String) -> Self {
        Self::error(id, error_codes::INVALID_PARAMS, message)
    }

    /// Create a pipeline not found error response
    pub fn pipeline_not_found(id: Value, pipeline_id: &str) -> Self {
        Self::error(
            id,
            error_codes::PIPELINE_NOT_FOUND,
            format!("Pipeline not found: {}", pipeline_id),
        )
    }

    /// Create a server error response from a GpopError
    pub fn from_gpop_error(id: Value, err: &crate::error::GpopError) -> Self {
        use crate::error::GpopError;

        let (code, message) = match err {
            GpopError::PipelineNotFound(pid) => (
                error_codes::PIPELINE_NOT_FOUND,
                format!("Pipeline not found: {}", pid),
            ),
            GpopError::InvalidPipeline(msg) => (error_codes::PIPELINE_CREATION_FAILED, msg.clone()),
            GpopError::StateChangeFailed(msg) => (error_codes::STATE_CHANGE_FAILED, msg.clone()),
            GpopError::MediaNotSupported(msg) => (error_codes::MEDIA_NOT_SUPPORTED, msg.clone()),
            GpopError::DiscoveryFailed(msg) => (error_codes::DISCOVERY_FAILED, msg.clone()),
            GpopError::GStreamer(msg) => (error_codes::GSTREAMER_ERROR, msg.clone()),
            _ => (error_codes::INTERNAL_ERROR, "Internal error".to_string()),
        };

        Self::error(id, code, message)
    }
}

// Manager-level request parameter types

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePipelineParams {
    pub description: String,
}

// Manager-level response result types

#[derive(Debug, Clone, Serialize)]
pub struct PipelineCreatedResult {
    pub pipeline_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListPipelinesResult {
    pub pipelines: Vec<super::pipeline::PipelineInfoResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionResult {
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InfoResult {
    pub daemon_version: String,
    pub gstreamer_version: String,
    pub jsonrpc_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineCountResult {
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetElementsParams {
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GetElementsResult {
    pub elements: Vec<crate::gst::registry::ElementInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscoverUriParams {
    pub uri: String,
    #[serde(default)]
    pub timeout: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverUriResult {
    #[serde(flatten)]
    pub info: crate::gst::discoverer::DiscoverResult,
}
