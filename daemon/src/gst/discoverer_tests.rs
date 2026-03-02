// discoverer_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::gst::discoverer::{
    build_playbin_description, discover_uri, normalize_uri, DEFAULT_TIMEOUT_SECS,
};

#[test]
fn test_discover_invalid_uri() {
    let _ = gstreamer::init();
    let result = discover_uri("file:///nonexistent/path/video.mp4", Some(5));
    assert!(result.is_err());
}

#[test]
fn test_discover_empty_uri() {
    let _ = gstreamer::init();
    let result = discover_uri("", Some(5));
    assert!(result.is_err());
}

#[test]
fn test_discover_bad_scheme() {
    let _ = gstreamer::init();
    // "not-a-valid-uri" has no scheme, so normalize_uri treats it as a relative
    // file path. The discoverer will fail because the file doesn't exist.
    let result = discover_uri("not-a-valid-uri", Some(5));
    assert!(result.is_err());
}

#[test]
fn test_discover_absolute_path() {
    let _ = gstreamer::init();
    let result = discover_uri("/nonexistent/path/video.mp4", Some(5));
    assert!(result.is_err());
}

#[test]
fn test_discover_relative_path() {
    let _ = gstreamer::init();
    let result = discover_uri("nonexistent/video.mp4", Some(5));
    assert!(result.is_err());
}

#[test]
fn test_default_timeout() {
    assert_eq!(DEFAULT_TIMEOUT_SECS, 10);
}

#[test]
fn test_normalize_uri_with_scheme() {
    let uri = normalize_uri("http://example.com/video.mp4").unwrap();
    assert_eq!(uri, "http://example.com/video.mp4");
}

#[test]
fn test_normalize_uri_file_scheme() {
    let uri = normalize_uri("file:///tmp/video.mp4").unwrap();
    assert_eq!(uri, "file:///tmp/video.mp4");
}

#[test]
fn test_normalize_uri_absolute_path() {
    let uri = normalize_uri("/tmp/video.mp4").unwrap();
    assert!(uri.starts_with("file://"));
    assert!(uri.contains("tmp"));
    assert!(uri.contains("video.mp4"));
}

#[test]
fn test_normalize_uri_relative_path() {
    let uri = normalize_uri("video.mp4").unwrap();
    assert!(uri.starts_with("file://"));
    assert!(uri.contains("video.mp4"));
    // Should contain an absolute path (resolved from cwd)
    // On Unix: file:///abs/path, on Windows: file:///C:/abs/path
    let path_part = if cfg!(windows) {
        uri.strip_prefix("file:///").unwrap()
    } else {
        uri.strip_prefix("file://").unwrap()
    };
    assert!(std::path::Path::new(path_part).is_absolute());
}

#[test]
fn test_normalize_uri_empty_string() {
    let result = normalize_uri("");
    assert!(result.is_err());
}

#[test]
fn test_normalize_uri_whitespace_only() {
    let result = normalize_uri("   ");
    assert!(result.is_err());
}

// --- build_playbin_description tests ---

#[test]
fn test_build_playbin_description_default_playbin3() {
    let desc = build_playbin_description("file:///test.mp4", None, None, false).unwrap();
    assert_eq!(desc, r#"playbin3 uri="file:///test.mp4""#);
}

#[test]
fn test_build_playbin_description_playbin2_fallback() {
    let desc = build_playbin_description("file:///test.mp4", None, None, true).unwrap();
    assert!(
        desc.starts_with("playbin "),
        "Should use 'playbin' not 'playbin3'"
    );
    assert!(desc.contains(r#"uri="file:///test.mp4""#));
}

#[test]
fn test_build_playbin_description_with_video_sink() {
    let desc =
        build_playbin_description("file:///test.mp4", Some("fakesink"), None, false).unwrap();
    assert_eq!(
        desc,
        r#"playbin3 uri="file:///test.mp4" video-sink=fakesink"#
    );
}

#[test]
fn test_build_playbin_description_with_audio_sink() {
    let desc =
        build_playbin_description("file:///test.mp4", None, Some("autoaudiosink"), false).unwrap();
    assert_eq!(
        desc,
        r#"playbin3 uri="file:///test.mp4" audio-sink=autoaudiosink"#
    );
}

#[test]
fn test_build_playbin_description_with_both_sinks() {
    let desc = build_playbin_description(
        "file:///test.mp4",
        Some("glimagesink"),
        Some("pulsesink"),
        false,
    )
    .unwrap();
    assert_eq!(
        desc,
        r#"playbin3 uri="file:///test.mp4" video-sink=glimagesink audio-sink=pulsesink"#
    );
}

#[test]
fn test_build_playbin_description_normalizes_relative_path() {
    let desc = build_playbin_description("video.mp4", None, None, false).unwrap();
    assert!(desc.starts_with("playbin3 "));
    assert!(desc.contains("file://"));
    assert!(desc.contains("video.mp4"));
}

#[test]
fn test_build_playbin_description_http_uri_passthrough() {
    let desc =
        build_playbin_description("http://example.com/stream.mp4", None, None, false).unwrap();
    assert_eq!(desc, r#"playbin3 uri="http://example.com/stream.mp4""#);
}

#[test]
fn test_build_playbin_description_rejects_injection_in_video_sink() {
    let result = build_playbin_description(
        "file:///test.mp4",
        Some("fakesink ! filesrc location=/etc/passwd"),
        None,
        false,
    );
    assert!(result.is_err());
}

#[test]
fn test_build_playbin_description_rejects_injection_in_audio_sink() {
    let result = build_playbin_description(
        "file:///test.mp4",
        None,
        Some(r#"fakesink" uri="http://evil.com"#),
        false,
    );
    assert!(result.is_err());
}

#[test]
fn test_build_playbin_description_rejects_empty_sink() {
    let result = build_playbin_description("file:///test.mp4", Some(""), None, false);
    assert!(result.is_err());
}

#[test]
fn test_build_playbin_description_empty_uri() {
    let result = build_playbin_description("", None, None, false);
    assert!(result.is_err());
}
