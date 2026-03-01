// event.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PipelineState {
    /// Pipeline is in void/pending state (transitioning)
    #[serde(rename = "void_pending")]
    VoidPending,
    Null,
    Ready,
    Paused,
    Playing,
}

impl std::fmt::Display for PipelineState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineState::VoidPending => write!(f, "void_pending"),
            PipelineState::Null => write!(f, "null"),
            PipelineState::Ready => write!(f, "ready"),
            PipelineState::Paused => write!(f, "paused"),
            PipelineState::Playing => write!(f, "playing"),
        }
    }
}

impl std::str::FromStr for PipelineState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "void_pending" | "voidpending" => Ok(PipelineState::VoidPending),
            "null" => Ok(PipelineState::Null),
            "ready" => Ok(PipelineState::Ready),
            "paused" => Ok(PipelineState::Paused),
            "playing" => Ok(PipelineState::Playing),
            _ => Err("Invalid state. Valid values: null, ready, paused, playing".to_string()),
        }
    }
}

impl From<gstreamer::State> for PipelineState {
    fn from(state: gstreamer::State) -> Self {
        match state {
            gstreamer::State::VoidPending => PipelineState::VoidPending,
            gstreamer::State::Null => PipelineState::Null,
            gstreamer::State::Ready => PipelineState::Ready,
            gstreamer::State::Paused => PipelineState::Paused,
            gstreamer::State::Playing => PipelineState::Playing,
        }
    }
}

impl From<PipelineState> for gstreamer::State {
    fn from(state: PipelineState) -> Self {
        match state {
            PipelineState::VoidPending => gstreamer::State::VoidPending,
            PipelineState::Null => gstreamer::State::Null,
            PipelineState::Ready => gstreamer::State::Ready,
            PipelineState::Paused => gstreamer::State::Paused,
            PipelineState::Playing => gstreamer::State::Playing,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum PipelineEvent {
    #[serde(rename = "state_changed")]
    StateChanged {
        pipeline_id: String,
        old_state: PipelineState,
        new_state: PipelineState,
    },
    #[serde(rename = "error")]
    Error {
        pipeline_id: String,
        message: String,
    },
    #[serde(rename = "unsupported")]
    Unsupported {
        pipeline_id: String,
        message: String,
    },
    #[serde(rename = "eos")]
    Eos { pipeline_id: String },
    #[serde(rename = "pipeline_added")]
    PipelineAdded {
        pipeline_id: String,
        description: String,
    },
    #[serde(rename = "pipeline_updated")]
    PipelineUpdated {
        pipeline_id: String,
        description: String,
    },
    #[serde(rename = "pipeline_removed")]
    PipelineRemoved { pipeline_id: String },
}

pub type EventSender = tokio::sync::broadcast::Sender<PipelineEvent>;
pub type EventReceiver = tokio::sync::broadcast::Receiver<PipelineEvent>;

pub fn create_event_channel() -> (EventSender, EventReceiver) {
    tokio::sync::broadcast::channel(256)
}
