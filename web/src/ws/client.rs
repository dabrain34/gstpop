use actix_ws::Session;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// Progress message sent to browser clients
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ProgressMessage {
    #[serde(rename = "job_started")]
    JobStarted { job_id: String },

    #[serde(rename = "progress")]
    Progress {
        job_id: String,
        progress: f64,
        position_ns: Option<u64>,
        duration_ns: Option<u64>,
    },

    #[serde(rename = "job_completed")]
    JobCompleted {
        job_id: String,
        download_url: String,
    },

    #[serde(rename = "demucs_completed")]
    DemucsCompleted { job_id: String },

    #[serde(rename = "job_failed")]
    JobFailed { job_id: String, error: String },

    #[serde(rename = "state_changed")]
    StateChanged {
        job_id: String,
        old_state: String,
        new_state: String,
    },
}

/// Broadcaster for progress messages to all connected browser clients
#[derive(Clone)]
pub struct ProgressBroadcaster {
    tx: broadcast::Sender<ProgressMessage>,
}

impl ProgressBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    /// Send a progress message to all connected clients
    pub fn send(&self, message: ProgressMessage) {
        if self.tx.send(message).is_err() {
            debug!("No progress receivers connected");
        }
    }

    /// Subscribe to progress messages
    pub fn subscribe(&self) -> broadcast::Receiver<ProgressMessage> {
        self.tx.subscribe()
    }
}

impl Default for ProgressBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a WebSocket connection from a browser client
pub async fn handle_client_websocket(mut session: Session, broadcaster: Arc<ProgressBroadcaster>) {
    let mut rx = broadcaster.subscribe();

    loop {
        tokio::select! {
            // Receive progress messages and forward to client
            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        let json = match serde_json::to_string(&msg) {
                            Ok(j) => j,
                            Err(e) => {
                                warn!("Failed to serialize progress message: {}", e);
                                continue;
                            }
                        };

                        if session.text(json).await.is_err() {
                            debug!("Client disconnected");
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Client lagged, skipped {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        debug!("Broadcaster closed");
                        break;
                    }
                }
            }
        }
    }

    debug!("Client WebSocket handler ended");
}
