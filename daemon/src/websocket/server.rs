// server.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use subtle::ConstantTimeEq;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::handshake::server::{
    ErrorResponse, Request as WsRequest, Response as WsResponse,
};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

use crate::error::Result;
use crate::gst::{EventReceiver, PipelineManager};

use super::manager::ManagerInterface;
use super::pipeline::SnapshotParams;
use super::protocol::Request;
use super::{CLIENT_MESSAGE_BUFFER, MAX_CONCURRENT_CLIENTS};

/// Maximum WebSocket message/frame size (128 KB) to prevent memory exhaustion
const MAX_WS_MESSAGE_SIZE: usize = 128 * 1024;

fn ws_config() -> WebSocketConfig {
    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(MAX_WS_MESSAGE_SIZE);
    config.max_frame_size = Some(MAX_WS_MESSAGE_SIZE);
    config
}

type ClientTx = mpsc::Sender<Message>;
type ClientMap = Arc<RwLock<HashMap<SocketAddr, ClientTx>>>;

/// Serialize a value to JSON, returning an error JSON response if serialization fails.
/// This should never fail for well-typed structs, but we handle it gracefully.
fn serialize_or_error<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| {
        error!("JSON serialization failed: {}", e);
        // Return a minimal valid JSON error response
        r#"{"jsonrpc":"2.0","id":"unknown","error":{"code":-32603,"message":"Internal serialization error"}}"#.to_string()
    })
}

pub struct WebSocketServer {
    addr: SocketAddr,
    manager: Arc<PipelineManager>,
    clients: ClientMap,
    api_key: Option<String>,
    allowed_origins: Option<Vec<String>>,
    /// Counter for dropped events (client buffer full or disconnected)
    dropped_events: Arc<AtomicU64>,
}

impl WebSocketServer {
    pub fn new(
        addr: SocketAddr,
        manager: Arc<PipelineManager>,
        api_key: Option<String>,
        allowed_origins: Option<Vec<String>>,
    ) -> Self {
        Self {
            addr,
            manager,
            clients: Arc::new(RwLock::new(HashMap::new())),
            api_key,
            allowed_origins,
            dropped_events: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get the total number of events dropped due to slow clients or disconnections.
    /// This counter is useful for monitoring and debugging.
    pub fn dropped_event_count(&self) -> u64 {
        self.dropped_events.load(Ordering::Relaxed)
    }

    pub async fn run(self, mut event_rx: EventReceiver) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        info!("WebSocket server listening on ws://{}", self.addr);

        let clients = Arc::clone(&self.clients);
        let manager = Arc::clone(&self.manager);
        let api_key = self.api_key.clone();
        let allowed_origins = self.allowed_origins.clone();
        let dropped_events = Arc::clone(&self.dropped_events);

        // Spawn event broadcaster
        let broadcast_clients = Arc::clone(&clients);
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        // Serialize once, then clone for each client
                        // Note: Message::Text requires owned String, so we must clone per-client
                        let msg = serialize_or_error(&event);
                        let clients = broadcast_clients.read().await;
                        for (addr, tx) in clients.iter() {
                            // Use try_send to avoid blocking; if buffer is full, client is slow
                            if tx.try_send(Message::Text(msg.clone().into())).is_err() {
                                dropped_events.fetch_add(1, Ordering::Relaxed);
                                debug!("Failed to send event to client {} (buffer full or disconnected)", addr);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WebSocket broadcaster lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        info!("Event channel closed, stopping WebSocket broadcaster");
                        break;
                    }
                }
            }
        });

        // Accept connections
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let clients = Arc::clone(&clients);
                    let manager = Arc::clone(&manager);
                    let api_key = api_key.clone();
                    let allowed_origins = allowed_origins.clone();
                    tokio::spawn(handle_connection(
                        stream,
                        addr,
                        clients,
                        manager,
                        api_key,
                        allowed_origins,
                    ));
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    pub fn clients(&self) -> ClientMap {
        Arc::clone(&self.clients)
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    clients: ClientMap,
    manager: Arc<PipelineManager>,
    api_key: Option<String>,
    allowed_origins: Option<Vec<String>>,
) {
    info!("New WebSocket connection from {}", addr);

    // Accept WebSocket connection with optional API key and origin validation
    let ws_stream = if api_key.is_some() || allowed_origins.is_some() {
        let expected_key = api_key;
        let origins = allowed_origins;
        let callback = move |req: &WsRequest,
                             res: WsResponse|
              -> std::result::Result<WsResponse, ErrorResponse> {
            // Validate Origin header if allowed_origins is configured
            // Note: Non-browser clients (CLI tools, scripts) typically don't send Origin headers.
            // If Origin is absent, we allow the request to support programmatic API access.
            // Only reject if Origin is present but not in the allowed list.
            if let Some(ref allowed) = origins {
                if let Some(origin_header) = req.headers().get("Origin") {
                    let origin = origin_header.to_str().unwrap_or("");
                    if !allowed.iter().any(|o| o == origin) {
                        warn!(
                            "Rejected connection: origin '{}' not in allowed list",
                            origin
                        );
                        let mut err = ErrorResponse::new(None);
                        *err.status_mut() = StatusCode::FORBIDDEN;
                        return Err(err);
                    }
                }
                // No Origin header = non-browser client, allow through
            }

            // Validate API key if configured
            if let Some(ref expected) = expected_key {
                match req.headers().get("Authorization") {
                    Some(value) => {
                        let provided = value.to_str().unwrap_or("").as_bytes();
                        let expected_bytes = expected.as_bytes();
                        // Use constant-time comparison to prevent timing attacks.
                        // Hash both values to normalize length before comparison,
                        // preventing key length leaks via timing.
                        use std::hash::{Hash, Hasher};
                        let hash_bytes = |data: &[u8]| -> u64 {
                            let mut hasher = std::collections::hash_map::DefaultHasher::new();
                            data.hash(&mut hasher);
                            hasher.finish()
                        };
                        // First check: constant-time comparison of hashes (fixed-size, no length leak)
                        let provided_hash = hash_bytes(provided).to_le_bytes();
                        let expected_hash = hash_bytes(expected_bytes).to_le_bytes();
                        let hashes_match = bool::from(provided_hash.ct_eq(&expected_hash));
                        // Second check: if lengths happen to match, also do a full ct_eq
                        // to avoid hash collision false positives
                        let is_valid = hashes_match
                            && provided.len() == expected_bytes.len()
                            && bool::from(provided.ct_eq(expected_bytes));
                        if is_valid {
                            return Ok(res);
                        } else {
                            let mut err = ErrorResponse::new(None);
                            *err.status_mut() = StatusCode::FORBIDDEN;
                            return Err(err);
                        }
                    }
                    None => {
                        let mut err = ErrorResponse::new(None);
                        *err.status_mut() = StatusCode::UNAUTHORIZED;
                        return Err(err);
                    }
                }
            }

            Ok(res)
        };
        match tokio_tungstenite::accept_hdr_async_with_config(stream, callback, Some(ws_config()))
            .await
        {
            Ok(ws) => ws,
            Err(e) => {
                error!("WebSocket handshake failed for {}: {}", addr, e);
                return;
            }
        }
    } else {
        match tokio_tungstenite::accept_async_with_config(stream, Some(ws_config())).await {
            Ok(ws) => ws,
            Err(e) => {
                error!("WebSocket handshake failed for {}: {}", addr, e);
                return;
            }
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::channel::<Message>(CLIENT_MESSAGE_BUFFER);

    // Register client (with limit check under single write lock to prevent TOCTOU)
    {
        let mut clients_map = clients.write().await;
        if clients_map.len() >= MAX_CONCURRENT_CLIENTS {
            warn!(
                "Max clients ({}) reached, rejecting connection from {}",
                MAX_CONCURRENT_CLIENTS, addr
            );
            return;
        }
        clients_map.insert(addr, tx);
    }

    let handler = ManagerInterface::new(manager);

    // Spawn task to forward messages from channel to WebSocket
    let sender_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                debug!("Received from {}: {}", addr, text);

                let request = match serde_json::from_str::<Request>(&text) {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Failed to parse request from {}: {}", addr, e);

                        // Try to extract the ID from malformed JSON for better error correlation
                        let id = serde_json::from_str::<serde_json::Value>(&text)
                            .ok()
                            .and_then(|v| v.get("id").cloned())
                            .unwrap_or(serde_json::Value::Null);

                        let response = super::protocol::Response::parse_error(
                            id,
                            "Invalid JSON-RPC request".to_string(),
                        );
                        let response_json = serialize_or_error(&response);
                        let clients_map = clients.read().await;
                        if let Some(tx) = clients_map.get(&addr) {
                            let _ = tx.try_send(Message::Text(response_json.into()));
                        }
                        continue;
                    }
                };

                // Handle snapshot specially (different params extraction)
                let response_json = if request.method == "snapshot" {
                    let params: SnapshotParams =
                        serde_json::from_value(request.params).unwrap_or_default();
                    let response = match handler.snapshot(params).await {
                        Ok(result) => match serde_json::to_value(&result) {
                            Ok(v) => super::protocol::Response::success(request.id, v),
                            Err(e) => {
                                error!("JSON serialization failed: {}", e);
                                super::protocol::Response::error(
                                    request.id,
                                    super::protocol::error_codes::INTERNAL_ERROR,
                                    "Internal serialization error".to_string(),
                                )
                            }
                        },
                        Err(e) => super::protocol::Response::from_gstpop_error(request.id, &e),
                    };
                    serialize_or_error(&response)
                } else {
                    let response = handler.handle(request).await;
                    serialize_or_error(&response)
                };

                let clients_map = clients.read().await;
                if let Some(tx) = clients_map.get(&addr) {
                    let _ = tx.try_send(Message::Text(response_json.into()));
                }
            }
            Ok(Message::Close(_)) => {
                info!("Client {} disconnected", addr);
                break;
            }
            Ok(Message::Ping(data)) => {
                let clients_map = clients.read().await;
                if let Some(tx) = clients_map.get(&addr) {
                    let _ = tx.try_send(Message::Pong(data));
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("Error receiving message from {}: {}", addr, e);
                break;
            }
        }
    }

    // Unregister client
    {
        let mut clients_map = clients.write().await;
        clients_map.remove(&addr);
    }

    sender_task.abort();
    info!("Connection closed for {}", addr);
}
