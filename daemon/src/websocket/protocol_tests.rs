// protocol_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use super::pipeline::*;
use super::protocol::*;
use crate::gst::PipelineState;

#[test]
fn test_request_deserialize() {
    let json = r#"{"id":"123","method":"list_pipelines","params":{}}"#;
    let request: Request = serde_json::from_str(json).unwrap();

    assert_eq!(request.id, serde_json::json!("123"));
    assert_eq!(request.method, "list_pipelines");
}

#[test]
fn test_request_deserialize_with_params() {
    let json = r#"{"id":"456","method":"create_pipeline","params":{"description":"videotestsrc ! fakesink"}}"#;
    let request: Request = serde_json::from_str(json).unwrap();

    assert_eq!(request.id, serde_json::json!("456"));
    assert_eq!(request.method, "create_pipeline");

    let params: CreatePipelineParams = serde_json::from_value(request.params).unwrap();
    assert_eq!(params.description, "videotestsrc ! fakesink");
}

#[test]
fn test_request_deserialize_optional_params() {
    let json = r#"{"id":"789","method":"list_pipelines"}"#;
    let request: Request = serde_json::from_str(json).unwrap();

    assert_eq!(request.id, serde_json::json!("789"));
    assert_eq!(request.method, "list_pipelines");
    assert!(request.params.is_null());
}

#[test]
fn test_request_numeric_id() {
    // JSON-RPC 2.0 allows numeric IDs
    let json = r#"{"id":42,"method":"list_pipelines"}"#;
    let request: Request = serde_json::from_str(json).unwrap();

    assert_eq!(request.id, serde_json::json!(42));
    assert_eq!(request.method, "list_pipelines");
}

#[test]
fn test_request_notification_no_id() {
    // JSON-RPC 2.0 notifications omit the id field
    let json = r#"{"method":"list_pipelines"}"#;
    let request: Request = serde_json::from_str(json).unwrap();

    assert!(request.id.is_null());
    assert_eq!(request.method, "list_pipelines");
}

#[test]
fn test_request_missing_method_fails() {
    // Per JSON-RPC 2.0, method is required
    let json = r#"{"id":"123"}"#;
    let result: Result<Request, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn test_invalid_request_response() {
    let response = Response::invalid_request(
        serde_json::json!("123"),
        "Missing required field".to_string(),
    );

    assert_eq!(response.id, serde_json::json!("123"));
    assert!(response.error.is_some());

    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::INVALID_REQUEST);
}

#[test]
fn test_response_success() {
    let response = Response::success(
        serde_json::json!("123"),
        serde_json::json!({"pipeline_id": "pipeline-0"}),
    );

    assert_eq!(response.id, serde_json::json!("123"));
    assert!(response.result.is_some());
    assert!(response.error.is_none());

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"pipeline_id\":\"pipeline-0\""));
    assert!(!json.contains("\"error\""));
}

#[test]
fn test_response_error() {
    let response = Response::error(
        serde_json::json!("123"),
        -32600,
        "Invalid request".to_string(),
    );

    assert_eq!(response.id, serde_json::json!("123"));
    assert!(response.result.is_none());
    assert!(response.error.is_some());

    let error = response.error.unwrap();
    assert_eq!(error.code, -32600);
    assert_eq!(error.message, "Invalid request");
}

#[test]
fn test_response_serialization_skips_none() {
    let success = Response::success(serde_json::json!("1"), serde_json::json!({}));
    let json = serde_json::to_string(&success).unwrap();
    assert!(!json.contains("\"error\""));

    let error = Response::error(
        serde_json::json!("2"),
        error_codes::INTERNAL_ERROR,
        "Error".to_string(),
    );
    let json = serde_json::to_string(&error).unwrap();
    assert!(!json.contains("\"result\""));
}

#[test]
fn test_response_echoes_numeric_id() {
    let response = Response::success(serde_json::json!(42), serde_json::json!({}));
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"id\":42"));
}

#[test]
fn test_create_pipeline_params() {
    let json = r#"{"description":"videotestsrc ! fakesink"}"#;
    let params: CreatePipelineParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.description, "videotestsrc ! fakesink");
}

#[test]
fn test_pipeline_id_params() {
    let json = r#"{"pipeline_id":"pipeline-0"}"#;
    let params: PipelineIdParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.pipeline_id, "pipeline-0");
}

#[test]
fn test_set_state_params() {
    let json = r#"{"pipeline_id":"pipeline-0","state":"playing"}"#;
    let params: SetStateParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.pipeline_id, "pipeline-0");
    assert_eq!(params.state, "playing");
}

#[test]
fn test_snapshot_params_with_details() {
    let json = r#"{"pipeline_id":"0","details":"all"}"#;
    let params: SnapshotParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.pipeline_id, Some("0".to_string()));
    assert_eq!(params.details, Some("all".to_string()));
}

#[test]
fn test_snapshot_params_without_details() {
    let json = r#"{"pipeline_id":"0"}"#;
    let params: SnapshotParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.pipeline_id, Some("0".to_string()));
    assert!(params.details.is_none());
}

#[test]
fn test_snapshot_params_empty() {
    let json = r#"{}"#;
    let params: SnapshotParams = serde_json::from_str(json).unwrap();
    assert!(params.pipeline_id.is_none());
    assert!(params.details.is_none());
}

#[test]
fn test_pipeline_info_result() {
    let result = PipelineInfoResult {
        id: "pipeline-0".to_string(),
        description: "videotestsrc ! fakesink".to_string(),
        state: PipelineState::Playing,
        streaming: true,
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"id\":\"pipeline-0\""));
    assert!(json.contains("\"state\":\"playing\""));
    assert!(json.contains("\"streaming\":true"));
}

#[test]
fn test_list_pipelines_result() {
    let result = ListPipelinesResult {
        pipelines: vec![
            PipelineInfoResult {
                id: "pipeline-0".to_string(),
                description: "test1".to_string(),
                state: PipelineState::Null,
                streaming: false,
            },
            PipelineInfoResult {
                id: "pipeline-1".to_string(),
                description: "test2".to_string(),
                state: PipelineState::Playing,
                streaming: true,
            },
        ],
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"pipelines\":["));
    assert!(json.contains("\"pipeline-0\""));
    assert!(json.contains("\"pipeline-1\""));
}

#[test]
fn test_success_result() {
    let result = SuccessResult { success: true };
    let json = serde_json::to_string(&result).unwrap();
    assert_eq!(json, r#"{"success":true}"#);
}

#[test]
fn test_snapshot_result() {
    let result = SnapshotResult {
        response_type: "SnapshotResponse".to_string(),
        pipelines: vec![PipelineSnapshot {
            id: "0".to_string(),
            dot: "digraph pipeline {}".to_string(),
        }],
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("digraph pipeline"));
    assert!(json.contains("\"type\":\"SnapshotResponse\""));
}

#[test]
fn test_pipeline_created_result() {
    let result = PipelineCreatedResult {
        pipeline_id: "pipeline-0".to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    assert_eq!(json, r#"{"pipeline_id":"pipeline-0"}"#);
}

#[test]
fn test_play_uri_params_minimal() {
    let json = r#"{"uri":"file:///test.mp4"}"#;
    let params: PlayUriParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.uri, "file:///test.mp4");
    assert!(params.video_sink.is_none());
    assert!(params.audio_sink.is_none());
    assert!(params.use_playbin2.is_none());
}

#[test]
fn test_play_uri_params_all_fields() {
    let json = r#"{
        "uri": "http://example.com/video.mp4",
        "video_sink": "fakesink",
        "audio_sink": "autoaudiosink",
        "use_playbin2": true
    }"#;
    let params: PlayUriParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.uri, "http://example.com/video.mp4");
    assert_eq!(params.video_sink, Some("fakesink".to_string()));
    assert_eq!(params.audio_sink, Some("autoaudiosink".to_string()));
    assert_eq!(params.use_playbin2, Some(true));
}

#[test]
fn test_play_uri_params_missing_uri_fails() {
    let json = r#"{"video_sink":"fakesink"}"#;
    let result: Result<PlayUriParams, _> = serde_json::from_str(json);
    assert!(result.is_err(), "URI is required");
}

#[test]
fn test_play_uri_result_serialization() {
    let result = PlayUriResult {
        pipeline_id: "42".to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    assert_eq!(json, r#"{"pipeline_id":"42"}"#);
}
