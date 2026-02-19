// lib.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(target_os = "linux")]
pub mod dbus;
pub mod error;
pub mod gst;
pub mod playback;
pub mod websocket;

pub use error::{GpopError, Result};
pub use gst::{
    create_event_channel, Pipeline, PipelineEvent, PipelineInfo, PipelineManager, PipelineState,
};
