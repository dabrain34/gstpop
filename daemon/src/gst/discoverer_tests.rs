// discoverer_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::gst::discoverer::{discover_uri, normalize_uri, DEFAULT_TIMEOUT_SECS};

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
