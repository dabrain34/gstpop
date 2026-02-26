// error.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpopError {
    #[error("GStreamer error: {0}")]
    GStreamer(String),

    #[error("Pipeline not found: {0}")]
    PipelineNotFound(String),

    #[error("Invalid pipeline description: {0}")]
    InvalidPipeline(String),

    #[error("State change failed: {0}")]
    StateChangeFailed(String),

    #[error("Media not supported: {0}")]
    MediaNotSupported(String),

    #[error("Discovery failed: {0}")]
    DiscoveryFailed(String),

    #[cfg(target_os = "linux")]
    #[error("DBus error: {0}")]
    DBus(#[from] zbus::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, GpopError>;
