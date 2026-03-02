// discoverer.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use gstreamer as gst;
use gstreamer_pbutils as gst_pbutils;
use gstreamer_pbutils::prelude::*;
use serde::Serialize;

use crate::error::{GstpopError, Result};

/// Default discovery timeout in seconds.
pub const DEFAULT_TIMEOUT_SECS: u32 = 10;

/// Top-level result of discovering a URI.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoverResult {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ns: Option<u64>,
    pub seekable: bool,
    pub live: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerInfo>,
    pub video_streams: Vec<VideoStreamInfo>,
    pub audio_streams: Vec<AudioStreamInfo>,
    pub subtitle_streams: Vec<SubtitleStreamInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<TagsInfo>,
}

/// Container format information.
#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub caps: String,
}

/// Video stream information.
#[derive(Debug, Clone, Serialize)]
pub struct VideoStreamInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    pub width: u32,
    pub height: u32,
    pub framerate_num: i32,
    pub framerate_denom: i32,
    pub bitrate: u32,
    pub max_bitrate: u32,
    pub depth: u32,
    pub is_interlaced: bool,
    pub is_image: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub par_num: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub par_denom: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
}

/// Audio stream information.
#[derive(Debug, Clone, Serialize)]
pub struct AudioStreamInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    pub channels: u32,
    pub sample_rate: u32,
    pub bitrate: u32,
    pub max_bitrate: u32,
    pub depth: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
}

/// Subtitle stream information.
#[derive(Debug, Clone, Serialize)]
pub struct SubtitleStreamInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
}

/// Selected tags from the media.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TagsInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
}

/// Convert a user-supplied URI or file path to a proper URI.
///
/// If the input already contains a URI scheme (e.g., `file://`, `http://`), it is
/// returned as-is. Otherwise it is treated as a file path: relative paths are
/// resolved against the current working directory and the result is converted to
/// a `file://` URI.
pub fn normalize_uri(uri: &str) -> Result<String> {
    let uri = uri.trim();
    if uri.is_empty() {
        return Err(GstpopError::DiscoveryFailed(
            "Empty URI provided".to_string(),
        ));
    }

    if uri.contains("://") {
        return Ok(uri.to_string());
    }

    let path = std::path::Path::new(uri);
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| GstpopError::DiscoveryFailed(format!("Cannot resolve path: {}", e)))?
            .join(path)
    };
    // Canonicalize to resolve .. and symlinks; fall back if file doesn't exist yet
    let abs = abs.canonicalize().unwrap_or(abs);

    // Use forward slashes and proper file URI format for cross-platform compatibility.
    // On Windows, abs.display() produces backslashes which are invalid in file URIs.
    // On Unix, paths start with / so file:// + /path gives file:///path.
    // On Windows, paths start with C:/ so we need file:///C:/path.
    let path_str = abs.to_string_lossy().replace('\\', "/");
    if path_str.starts_with('/') {
        Ok(format!("file://{}", path_str))
    } else {
        Ok(format!("file:///{}", path_str))
    }
}

/// Validate that a sink element name does not contain characters that could
/// inject additional elements or properties into the pipeline description.
fn validate_sink_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(GstpopError::InvalidPipeline(
            "Empty sink element name".to_string(),
        ));
    }
    if name.contains('!')
        || name.contains('"')
        || name.contains('\'')
        || name.contains('`')
        || name.contains('\\')
    {
        return Err(GstpopError::InvalidPipeline(format!(
            "Invalid sink element name: {}",
            name
        )));
    }
    Ok(())
}

/// Build a playbin pipeline description from a URI and optional sink overrides.
///
/// Uses `playbin3` by default. Set `use_playbin2` to `true` to fall back to
/// the legacy `playbin` element.
/// The URI is normalized (bare file paths are converted to `file://` URIs).
/// Optional `video_sink` and `audio_sink` parameters override the default sinks.
pub fn build_playbin_description(
    uri: &str,
    video_sink: Option<&str>,
    audio_sink: Option<&str>,
    use_playbin2: bool,
) -> Result<String> {
    let uri = normalize_uri(uri)?;
    if let Some(vs) = video_sink {
        validate_sink_name(vs)?;
    }
    if let Some(a_s) = audio_sink {
        validate_sink_name(a_s)?;
    }
    let element = if use_playbin2 { "playbin" } else { "playbin3" };
    let escaped_uri = uri.replace('"', r#"\""#);
    let mut desc = format!("{} uri=\"{}\"", element, escaped_uri);
    if let Some(vs) = video_sink {
        desc.push_str(&format!(" video-sink={}", vs));
    }
    if let Some(a_s) = audio_sink {
        desc.push_str(&format!(" audio-sink={}", a_s));
    }
    Ok(desc)
}

/// Discover media information for a given URI or file path.
///
/// This is a blocking operation that creates an internal GStreamer pipeline.
/// The `timeout_secs` parameter controls how long to wait (default: 10 seconds).
///
/// Both proper URIs (`file:///path`, `http://…`) and plain file paths
/// (`/absolute/path` or `relative/path`) are accepted.
pub fn discover_uri(uri: &str, timeout_secs: Option<u32>) -> Result<DiscoverResult> {
    let uri = normalize_uri(uri)?;

    let timeout = timeout_secs
        .filter(|&t| t > 0)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    let timeout_ns = gst::ClockTime::from_seconds(timeout as u64);

    let discoverer = gst_pbutils::Discoverer::new(timeout_ns)
        .map_err(|e| GstpopError::GStreamer(format!("Failed to create discoverer: {}", e)))?;

    let info = discoverer.discover_uri(&uri).map_err(|e| {
        GstpopError::DiscoveryFailed(format!("Discovery failed for '{}': {}", uri, e))
    })?;

    Ok(build_discover_result(&info))
}

fn build_discover_result(info: &gst_pbutils::DiscovererInfo) -> DiscoverResult {
    let uri = info.uri().to_string();
    let duration_ns = info.duration().map(|d| d.nseconds());
    let seekable = info.is_seekable();
    let live = info.is_live();

    let container = info.container_streams().first().map(|c| {
        let caps_str = c.caps().map(|caps| caps.to_string()).unwrap_or_default();
        ContainerInfo { caps: caps_str }
    });

    let video_streams = info
        .video_streams()
        .iter()
        .map(build_video_stream_info)
        .collect();

    let audio_streams = info
        .audio_streams()
        .iter()
        .map(build_audio_stream_info)
        .collect();

    let subtitle_streams = info
        .subtitle_streams()
        .iter()
        .map(build_subtitle_stream_info)
        .collect();

    // Collect tags from streams since DiscovererInfo::tags() is deprecated since 1.20
    let tags = collect_tags(info);

    DiscoverResult {
        uri,
        duration_ns,
        seekable,
        live,
        container,
        video_streams,
        audio_streams,
        subtitle_streams,
        tags,
    }
}

fn extract_codec(stream: &impl gst_pbutils::prelude::DiscovererStreamInfoExt) -> Option<String> {
    stream.caps().and_then(|caps| {
        if caps.is_empty() {
            None
        } else {
            caps.structure(0).map(|s| s.name().to_string())
        }
    })
}

fn extract_stream_id(
    stream: &impl gst_pbutils::prelude::DiscovererStreamInfoExt,
) -> Option<String> {
    stream.stream_id().map(|s| s.to_string())
}

fn build_video_stream_info(v: &gst_pbutils::DiscovererVideoInfo) -> VideoStreamInfo {
    let fr = v.framerate();
    let par = v.par();
    // Treat PAR as a unit: both numerator and denominator must be valid
    let (par_num, par_denom) = if par.numer() != 0 && par.denom() != 0 {
        (Some(par.numer()), Some(par.denom()))
    } else {
        (None, None)
    };
    VideoStreamInfo {
        codec: extract_codec(v),
        width: v.width(),
        height: v.height(),
        framerate_num: fr.numer(),
        framerate_denom: fr.denom(),
        bitrate: v.bitrate(),
        max_bitrate: v.max_bitrate(),
        depth: v.depth(),
        is_interlaced: v.is_interlaced(),
        is_image: v.is_image(),
        par_num,
        par_denom,
        stream_id: extract_stream_id(v),
    }
}

fn build_audio_stream_info(a: &gst_pbutils::DiscovererAudioInfo) -> AudioStreamInfo {
    AudioStreamInfo {
        codec: extract_codec(a),
        channels: a.channels(),
        sample_rate: a.sample_rate(),
        bitrate: a.bitrate(),
        max_bitrate: a.max_bitrate(),
        depth: a.depth(),
        language: a.language().map(|s| s.to_string()),
        stream_id: extract_stream_id(a),
    }
}

fn build_subtitle_stream_info(s: &gst_pbutils::DiscovererSubtitleInfo) -> SubtitleStreamInfo {
    SubtitleStreamInfo {
        codec: extract_codec(s),
        language: s.language().map(|l| l.to_string()),
        stream_id: extract_stream_id(s),
    }
}

/// Collect tags from all streams. Merges tags across streams to extract global metadata.
fn collect_tags(info: &gst_pbutils::DiscovererInfo) -> Option<TagsInfo> {
    let mut merged = gst::TagList::new();

    for stream in info.stream_list() {
        if let Some(tags) = stream.tags() {
            // Safety: `merged` is locally owned with no other references
            let merged_mut = merged
                .get_mut()
                .expect("TagList should have unique ownership");
            merged_mut.merge(&tags, gst::TagMergeMode::Keep);
        }
    }

    if merged.n_tags() == 0 {
        return None;
    }

    let tags_info = TagsInfo {
        title: merged
            .get::<gst::tags::Title>()
            .map(|v| v.get().to_string()),
        artist: merged
            .get::<gst::tags::Artist>()
            .map(|v| v.get().to_string()),
        album: merged
            .get::<gst::tags::Album>()
            .map(|v| v.get().to_string()),
        genre: merged
            .get::<gst::tags::Genre>()
            .map(|v| v.get().to_string()),
        comment: merged
            .get::<gst::tags::Comment>()
            .map(|v| v.get().to_string()),
        container_format: merged
            .get::<gst::tags::ContainerFormat>()
            .map(|v| v.get().to_string()),
        encoder: merged
            .get::<gst::tags::Encoder>()
            .map(|v| v.get().to_string()),
    };

    // Only return if at least one tag is present
    if tags_info == TagsInfo::default() {
        None
    } else {
        Some(tags_info)
    }
}
