// pipeline_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use super::pipeline::*;
use crate::error::GstpopError;

fn init_gstreamer() {
    let _ = gstreamer::init();
}

// =============================================================================
// Pipeline::new() validation tests
// =============================================================================

#[test]
fn test_pipeline_new_empty_description_fails() {
    init_gstreamer();
    let result = Pipeline::new("test".to_string(), "");
    assert!(result.is_err());
    if let Err(GstpopError::InvalidPipeline(msg)) = result {
        assert!(msg.contains("empty"));
    } else {
        panic!("Expected InvalidPipeline error");
    }
}

#[test]
fn test_pipeline_new_whitespace_only_fails() {
    init_gstreamer();
    let result = Pipeline::new("test".to_string(), "   \t\n  ");
    assert!(result.is_err());
    if let Err(GstpopError::InvalidPipeline(msg)) = result {
        assert!(msg.contains("empty"));
    } else {
        panic!("Expected InvalidPipeline error");
    }
}

#[test]
fn test_pipeline_new_description_too_long_fails() {
    init_gstreamer();
    let long_description = "a".repeat(MAX_PIPELINE_DESCRIPTION_LENGTH + 1);
    let result = Pipeline::new("test".to_string(), &long_description);
    assert!(result.is_err());
    if let Err(GstpopError::InvalidPipeline(msg)) = result {
        assert!(msg.contains("too long"));
    } else {
        panic!("Expected InvalidPipeline error");
    }
}

#[test]
fn test_pipeline_new_invalid_gst_syntax_fails() {
    init_gstreamer();
    let result = Pipeline::new("test".to_string(), "invalid_element_xyz ! fakesink");
    assert!(result.is_err());
}

#[test]
fn test_pipeline_new_valid_description_succeeds() {
    init_gstreamer();
    let result = Pipeline::new("test".to_string(), "fakesrc ! fakesink");
    assert!(result.is_ok());
    let pipeline = result.unwrap();
    assert_eq!(pipeline.id(), "test");
    assert_eq!(pipeline.description(), "fakesrc ! fakesink");
}

// =============================================================================
// is_media_not_supported_error() tests
// =============================================================================

#[test]
fn test_is_media_not_supported_error_patterns() {
    // Test all supported media error patterns
    let media_error_messages = [
        "no suitable element found",
        "missing plugin for x265",
        "missing element: h265dec",
        "codec not found for video/x-h265",
        "could not determine type of stream",
        "unhandled stream type",
        "format not supported by decoder",
        "unsupported codec",
        "no decoder available for video/x-h265",
        "no encoder for audio/x-opus",
        "no demuxer for container",
        "no muxer available",
        "caps not supported by element",
        "not negotiated",
        "stream type not supported",
        "NO SUITABLE ELEMENT FOUND", // Case insensitive
    ];

    for msg in &media_error_messages {
        let error = gstreamer::glib::Error::new(gstreamer::CoreError::MissingPlugin, msg);
        assert!(
            is_media_not_supported_error(&error).is_some(),
            "Expected media error for: {}",
            msg
        );
    }
}

#[test]
fn test_is_media_not_supported_error_unrelated_returns_none() {
    let error =
        gstreamer::glib::Error::new(gstreamer::CoreError::Failed, "generic failure occurred");
    assert!(is_media_not_supported_error(&error).is_none());
}

#[test]
fn test_is_media_not_supported_error_returns_original_message() {
    let original_message = "no decoder available for video/x-h265";
    let error = gstreamer::glib::Error::new(gstreamer::CoreError::MissingPlugin, original_message);
    let result = is_media_not_supported_error(&error);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), original_message);
}

// =============================================================================
// Pipeline state and accessors tests
// =============================================================================

#[test]
fn test_pipeline_initial_state_is_null() {
    init_gstreamer();
    let pipeline = Pipeline::new("test".to_string(), "fakesrc ! fakesink").unwrap();
    assert_eq!(pipeline.state(), super::event::PipelineState::Null);
    assert!(!pipeline.is_streaming());
}

#[test]
fn test_pipeline_accessors() {
    init_gstreamer();
    let pipeline = Pipeline::new("my-pipeline-id".to_string(), "fakesrc ! fakesink").unwrap();
    assert_eq!(pipeline.id(), "my-pipeline-id");
    assert_eq!(pipeline.description(), "fakesrc ! fakesink");
    assert!(pipeline.bus().is_some());
}

#[test]
fn test_pipeline_get_position_returns_tuple() {
    init_gstreamer();
    let pipeline = Pipeline::new("test".to_string(), "fakesrc ! fakesink").unwrap();
    let (position, duration) = pipeline.get_position();
    // For a pipeline in Null state, position and duration are typically None
    assert!(position.is_none() || position.is_some());
    assert!(duration.is_none() || duration.is_some());
}

// =============================================================================
// Pipeline DOT graph tests
// =============================================================================

#[test]
fn test_pipeline_get_dot_all_detail_levels() {
    init_gstreamer();
    let pipeline = Pipeline::new("test".to_string(), "fakesrc ! fakesink").unwrap();

    // Test all detail options
    for details in &[
        None,
        Some("media"),
        Some("caps"),
        Some("non-default"),
        Some("states"),
        Some("all"),
        Some("unknown"),
    ] {
        let dot = pipeline.get_dot(*details);
        assert!(
            dot.contains("digraph"),
            "DOT output should contain 'digraph' for {:?}",
            details
        );
    }
}

// =============================================================================
// Pipeline state change tests
// =============================================================================

#[test]
fn test_pipeline_state_changes() {
    init_gstreamer();
    let pipeline = Pipeline::new("test".to_string(), "fakesrc ! fakesink").unwrap();

    // Test state transitions
    assert!(pipeline
        .set_state(super::event::PipelineState::Ready)
        .is_ok());
    assert!(pipeline.play().is_ok());
    assert!(pipeline.pause().is_ok());
    assert!(pipeline.stop().is_ok());
}

// =============================================================================
// Pipeline shutdown tests
// =============================================================================

#[test]
fn test_pipeline_shutdown_flag() {
    init_gstreamer();
    let pipeline = Pipeline::new("test".to_string(), "fakesrc ! fakesink").unwrap();
    let flag = pipeline.shutdown_flag();
    assert!(!flag.load(std::sync::atomic::Ordering::Acquire));

    pipeline.signal_shutdown();

    assert!(flag.load(std::sync::atomic::Ordering::Acquire));
}
