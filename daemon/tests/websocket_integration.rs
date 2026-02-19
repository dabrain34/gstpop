// websocket_integration.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

//! Integration tests for WebSocket server bounded channels and connection limits

use gpop::websocket::{CLIENT_MESSAGE_BUFFER, DEFAULT_WEBSOCKET_PORT, MAX_CONCURRENT_CLIENTS};

#[test]
fn test_client_message_buffer_is_bounded() {
    // Verify the constant is set to 256 (matching event channel buffer)
    assert_eq!(
        CLIENT_MESSAGE_BUFFER, 256,
        "CLIENT_MESSAGE_BUFFER should be 256 to match event channel buffer size"
    );
}

#[test]
fn test_max_concurrent_clients_is_reasonable() {
    // Verify the constant is set to a reasonable value for production use
    assert_eq!(
        MAX_CONCURRENT_CLIENTS, 1000,
        "MAX_CONCURRENT_CLIENTS should be 1000"
    );
}

#[test]
fn test_constants_are_public() {
    // This test verifies that the constants are exported and can be used
    // by downstream code if needed
    let _ = CLIENT_MESSAGE_BUFFER;
    let _ = MAX_CONCURRENT_CLIENTS;
    let _ = DEFAULT_WEBSOCKET_PORT;
}

// Note: Full WebSocket integration tests with actual connections require
// spinning up a server, which needs GStreamer initialized. These tests
// verify the configuration and protocol handling logic.

#[cfg(test)]
mod protocol_validation_tests {
    use gpop::websocket::protocol::{error_codes, Request, Response, JSONRPC_VERSION};

    #[test]
    fn test_request_without_id_is_notification() {
        // JSON-RPC 2.0: missing id = notification (id defaults to null)
        let json = r#"{"method":"list_pipelines"}"#;
        let result: Result<Request, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "Request without 'id' should parse as notification"
        );
        let request = result.unwrap();
        assert!(request.id.is_null());
    }

    #[test]
    fn test_request_requires_method_field() {
        // JSON-RPC 2.0 requires 'method'
        let json = r#"{"id":"123"}"#;
        let result: Result<Request, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "Request should fail without 'method' field"
        );
    }

    #[test]
    fn test_valid_request_parses() {
        let json = r#"{"id":"123","method":"list_pipelines"}"#;
        let result: Result<Request, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Valid request should parse successfully");

        let request = result.unwrap();
        assert_eq!(request.id, serde_json::json!("123"));
        assert_eq!(request.method, "list_pipelines");
        assert_eq!(request.jsonrpc, JSONRPC_VERSION);
    }

    #[test]
    fn test_response_success_format() {
        let response = Response::success(
            serde_json::json!("test-id"),
            serde_json::json!({"data": "value"}),
        );

        assert_eq!(response.id, serde_json::json!("test-id"));
        assert_eq!(response.jsonrpc, JSONRPC_VERSION);
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        // Verify serialization doesn't include null error
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_response_error_format() {
        let response = Response::error(
            serde_json::json!("test-id"),
            error_codes::INVALID_REQUEST,
            "Missing field".to_string(),
        );

        assert_eq!(response.id, serde_json::json!("test-id"));
        assert!(response.error.is_some());
        assert!(response.result.is_none());

        let error = response.error.as_ref().unwrap();
        assert_eq!(error.code, error_codes::INVALID_REQUEST);

        // Verify serialization doesn't include null result
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_invalid_request_helper() {
        let response = Response::invalid_request(
            serde_json::json!("req-1"),
            "Missing required field: method".to_string(),
        );

        let error = response.error.unwrap();
        assert_eq!(error.code, error_codes::INVALID_REQUEST);
        assert!(error.message.contains("method"));
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_all_error_codes_are_negative() {
        // JSON-RPC 2.0 spec requires error codes in specific ranges
        assert!(error_codes::PARSE_ERROR < 0);
        assert!(error_codes::INVALID_REQUEST < 0);
        assert!(error_codes::METHOD_NOT_FOUND < 0);
        assert!(error_codes::INVALID_PARAMS < 0);
        assert!(error_codes::INTERNAL_ERROR < 0);
        assert!(error_codes::PIPELINE_NOT_FOUND < 0);
        assert!(error_codes::PIPELINE_CREATION_FAILED < 0);
        assert!(error_codes::STATE_CHANGE_FAILED < 0);
        assert!(error_codes::GSTREAMER_ERROR < 0);
        assert!(error_codes::MEDIA_NOT_SUPPORTED < 0);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_server_error_codes_in_valid_range() {
        // JSON-RPC 2.0 reserves -32000 to -32099 for server errors
        let server_codes = [
            error_codes::PIPELINE_NOT_FOUND,
            error_codes::PIPELINE_CREATION_FAILED,
            error_codes::STATE_CHANGE_FAILED,
            error_codes::GSTREAMER_ERROR,
            error_codes::DESCRIPTION_TOO_LONG,
            error_codes::MEDIA_NOT_SUPPORTED,
        ];

        for code in server_codes {
            assert!(
                (-32099..=-32000).contains(&code),
                "Server error code {} should be in range -32099 to -32000",
                code
            );
        }
    }
}

#[cfg(test)]
mod origin_validation_tests {
    // These tests verify the origin validation logic behavior
    // The actual validation happens in the WebSocket handshake callback

    #[test]
    fn test_origin_matching_logic() {
        let allowed_origins = [
            "http://localhost:3000".to_string(),
            "https://example.com".to_string(),
        ];

        // Simulate the origin matching logic from server.rs
        let check_origin = |origin: &str| -> bool { allowed_origins.iter().any(|o| o == origin) };

        assert!(check_origin("http://localhost:3000"));
        assert!(check_origin("https://example.com"));
        assert!(!check_origin("http://evil.com"));
        assert!(!check_origin("https://localhost:3000")); // Different scheme
        assert!(!check_origin("")); // Empty origin
    }

    #[test]
    fn test_empty_allowed_origins_allows_none() {
        let allowed_origins: Vec<String> = vec![];

        // With empty allowed list, no origins match
        let check_origin = |origin: &str| -> bool { allowed_origins.iter().any(|o| o == origin) };

        assert!(!check_origin("http://localhost:3000"));
        assert!(!check_origin(""));
    }
}
