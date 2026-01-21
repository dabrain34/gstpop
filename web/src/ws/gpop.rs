use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::error::{AppError, Result};

/// Events received from gpop-daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum GpopEvent {
    #[serde(rename = "state_changed")]
    StateChanged {
        pipeline_id: String,
        old_state: String,
        new_state: String,
    },
    #[serde(rename = "error")]
    Error {
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
    #[serde(rename = "pipeline_removed")]
    PipelineRemoved { pipeline_id: String },
}

/// Response from gpop-daemon for JSON-RPC requests
#[derive(Debug, Clone, Deserialize)]
struct GpopResponse {
    id: String,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<GpopErrorInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct GpopErrorInfo {
    code: i32,
    message: String,
}

/// Position query result
#[derive(Debug, Clone, Deserialize)]
pub struct PositionResult {
    pub position_ns: Option<u64>,
    pub duration_ns: Option<u64>,
    pub progress: Option<f64>,
}

/// Pipeline creation result
#[derive(Debug, Clone, Deserialize)]
pub struct PipelineCreatedResult {
    pub pipeline_id: String,
}

/// Internal request to the writer task
struct GpopRequest {
    message: String,
    response_tx: oneshot::Sender<std::result::Result<Value, String>>,
}

/// Connection to gpop-daemon via WebSocket
pub struct GpopConnection {
    request_tx: mpsc::Sender<GpopRequest>,
    event_tx: broadcast::Sender<GpopEvent>,
    connected: Arc<RwLock<bool>>,
}

impl GpopConnection {
    /// Connect to gpop-daemon at the given URL with retry
    pub async fn connect(url: &str) -> Result<Self> {
        let mut last_err = String::new();
        let retry_delays = [100, 200, 400, 800, 1600];

        for (attempt, delay_ms) in std::iter::once(&0u64)
            .chain(retry_delays.iter())
            .enumerate()
        {
            if attempt > 0 {
                warn!(
                    "Retrying gpop connection (attempt {}/{}), waiting {}ms...",
                    attempt + 1,
                    retry_delays.len() + 1,
                    delay_ms
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(*delay_ms)).await;
            }

            match connect_async(url).await {
                Ok((ws_stream, _)) => {
                    info!("Connected to gpop-daemon at {}", url);
                    return Self::setup_connection(ws_stream).await;
                }
                Err(e) => {
                    last_err = e.to_string();
                    warn!("Failed to connect to gpop-daemon: {}", last_err);
                }
            }
        }

        Err(AppError::GpopConnection(format!(
            "Failed to connect after {} attempts: {}",
            retry_delays.len() + 1,
            last_err
        )))
    }

    async fn setup_connection(
        ws_stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Result<Self> {
        let (write, read) = ws_stream.split();

        let (request_tx, request_rx) = mpsc::channel::<GpopRequest>(32);
        let (event_tx, _) = broadcast::channel::<GpopEvent>(256);
        let connected = Arc::new(RwLock::new(true));

        // Pending requests waiting for responses
        let pending: Arc<
            RwLock<HashMap<String, oneshot::Sender<std::result::Result<Value, String>>>>,
        > = Arc::new(RwLock::new(HashMap::new()));

        // Spawn writer task
        let pending_clone = Arc::clone(&pending);
        let connected_clone = Arc::clone(&connected);
        tokio::spawn(async move {
            Self::writer_loop(write, request_rx, pending_clone, connected_clone).await;
        });

        // Spawn reader task
        let event_tx_clone = event_tx.clone();
        let pending_clone = Arc::clone(&pending);
        let connected_clone = Arc::clone(&connected);
        tokio::spawn(async move {
            Self::reader_loop(read, event_tx_clone, pending_clone, connected_clone).await;
        });

        Ok(Self {
            request_tx,
            event_tx,
            connected,
        })
    }

    async fn writer_loop(
        mut write: futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >,
        mut request_rx: mpsc::Receiver<GpopRequest>,
        pending: Arc<RwLock<HashMap<String, oneshot::Sender<std::result::Result<Value, String>>>>>,
        connected: Arc<RwLock<bool>>,
    ) {
        while let Some(request) = request_rx.recv().await {
            // Extract request ID from the message
            if let Ok(parsed) = serde_json::from_str::<Value>(&request.message) {
                if let Some(id) = parsed.get("id").and_then(|v| v.as_str()) {
                    // Store the response channel
                    pending
                        .write()
                        .await
                        .insert(id.to_string(), request.response_tx);

                    // Send the message
                    if let Err(e) = write.send(Message::Text(request.message.into())).await {
                        error!("Failed to send message to gpop: {}", e);
                        *connected.write().await = false;
                        break;
                    }
                } else {
                    let _ = request
                        .response_tx
                        .send(Err("Invalid request: missing id".to_string()));
                }
            } else {
                let _ = request
                    .response_tx
                    .send(Err("Invalid request: not valid JSON".to_string()));
            }
        }

        debug!("Writer loop ended");
    }

    async fn reader_loop(
        mut read: futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
        event_tx: broadcast::Sender<GpopEvent>,
        pending: Arc<RwLock<HashMap<String, oneshot::Sender<std::result::Result<Value, String>>>>>,
        connected: Arc<RwLock<bool>>,
    ) {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Disambiguate: peek at the JSON to determine message type
                    // Events have an "event" key, responses have an "id" key
                    if let Ok(raw) = serde_json::from_str::<Value>(&text) {
                        if raw.get("event").is_some() {
                            // This is an event broadcast
                            if let Ok(event) = serde_json::from_value::<GpopEvent>(raw) {
                                if event_tx.send(event).is_err() {
                                    debug!("No event receivers");
                                }
                            } else {
                                warn!("Failed to parse gpop event: {}", text);
                            }
                        } else if raw.get("id").is_some() {
                            // This is a response to a request
                            if let Ok(response) = serde_json::from_value::<GpopResponse>(raw) {
                                if let Some(tx) = pending.write().await.remove(&response.id) {
                                    let result = if let Some(error) = response.error {
                                        warn!(
                                            "gpop-daemon error (code {}): {}",
                                            error.code, error.message
                                        );
                                        Err(error.message)
                                    } else {
                                        Ok(response.result.unwrap_or(Value::Null))
                                    };
                                    let _ = tx.send(result);
                                }
                            }
                        } else {
                            warn!("Unknown message from gpop: {}", text);
                        }
                    } else {
                        warn!("Invalid JSON from gpop: {}", text);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("gpop connection closed");
                    *connected.write().await = false;
                    break;
                }
                Ok(Message::Ping(data)) => {
                    debug!("Received ping from gpop");
                    // Pong is handled automatically by tungstenite
                    let _ = data;
                }
                Err(e) => {
                    error!("Error reading from gpop: {}", e);
                    *connected.write().await = false;
                    break;
                }
                _ => {}
            }
        }

        debug!("Reader loop ended");
    }

    /// Send a JSON-RPC request and wait for the response with a 30s timeout
    async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = uuid::Uuid::new_v4().to_string();
        let message = json!({
            "id": id,
            "method": method,
            "params": params
        })
        .to_string();

        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(GpopRequest {
                message,
                response_tx,
            })
            .await
            .map_err(|_| AppError::GpopConnection("Connection closed".to_string()))?;

        match tokio::time::timeout(tokio::time::Duration::from_secs(30), response_rx).await {
            Ok(Ok(result)) => result.map_err(AppError::GpopProtocol),
            Ok(Err(_)) => Err(AppError::GpopConnection(
                "Response channel closed".to_string(),
            )),
            Err(_) => Err(AppError::GpopConnection(format!(
                "Request '{}' timed out after 30s",
                method
            ))),
        }
    }

    /// Create a new pipeline in gpop-daemon
    pub async fn create_pipeline(&self, description: &str) -> Result<String> {
        let result = self
            .request("create_pipeline", json!({ "description": description }))
            .await?;

        let created: PipelineCreatedResult =
            serde_json::from_value(result).map_err(|e| AppError::GpopProtocol(e.to_string()))?;

        Ok(created.pipeline_id)
    }

    /// Start playing a pipeline
    pub async fn play(&self, pipeline_id: &str) -> Result<()> {
        self.request("play", json!({ "pipeline_id": pipeline_id }))
            .await?;
        Ok(())
    }

    /// Pause a pipeline
    pub async fn pause(&self, pipeline_id: &str) -> Result<()> {
        self.request("pause", json!({ "pipeline_id": pipeline_id }))
            .await?;
        Ok(())
    }

    /// Stop a pipeline
    pub async fn stop(&self, pipeline_id: &str) -> Result<()> {
        self.request("stop", json!({ "pipeline_id": pipeline_id }))
            .await?;
        Ok(())
    }

    /// Remove a pipeline
    pub async fn remove_pipeline(&self, pipeline_id: &str) -> Result<()> {
        self.request("remove_pipeline", json!({ "pipeline_id": pipeline_id }))
            .await?;
        Ok(())
    }

    /// Get the current position and duration of a pipeline
    pub async fn get_position(&self, pipeline_id: &str) -> Result<PositionResult> {
        let result = self
            .request("get_position", json!({ "pipeline_id": pipeline_id }))
            .await?;

        serde_json::from_value(result).map_err(|e| AppError::GpopProtocol(e.to_string()))
    }

    /// Subscribe to events from gpop-daemon
    pub fn subscribe_events(&self) -> broadcast::Receiver<GpopEvent> {
        self.event_tx.subscribe()
    }

    /// Check if connected to gpop-daemon
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
}
