// main.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const REQUEST_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Serialize)]
struct Request {
    id: String,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    id: Value,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<ErrorInfo>,
}

#[derive(Debug, Deserialize)]
struct ErrorInfo {
    code: i32,
    message: String,
}

#[derive(Parser, Debug)]
#[command(name = "gst-popctl")]
#[command(author = "Stéphane Cerveau")]
#[command(version)]
#[command(about = "Control a gst-pop daemon via WebSocket")]
struct Cli {
    /// WebSocket URL of the gst-pop daemon
    #[arg(long, default_value = "ws://127.0.0.1:9000", global = true)]
    url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List all pipelines
    List,

    /// Create a new pipeline
    Create {
        /// Pipeline description
        description: String,
    },

    /// Update an existing pipeline description
    Update {
        /// Pipeline ID
        id: String,
        /// New pipeline description
        description: String,
    },

    /// Remove a pipeline
    Remove {
        /// Pipeline ID
        id: String,
    },

    /// Get pipeline info
    Info {
        /// Pipeline ID
        id: String,
    },

    /// Play a pipeline
    Play {
        /// Pipeline ID (default: all)
        id: Option<String>,
    },

    /// Pause a pipeline
    Pause {
        /// Pipeline ID (default: all)
        id: Option<String>,
    },

    /// Stop a pipeline
    Stop {
        /// Pipeline ID (default: all)
        id: Option<String>,
    },

    /// Set pipeline state
    State {
        /// Pipeline ID
        id: String,
        /// Target state (null, ready, paused, playing)
        state: String,
    },

    /// Get DOT graph snapshot of a pipeline
    Snapshot {
        /// Pipeline ID
        id: String,
        /// Detail level (media, caps, states, all)
        #[arg(long)]
        details: Option<String>,
    },

    /// Get pipeline position and duration
    Position {
        /// Pipeline ID (default: 0)
        id: Option<String>,
    },

    /// Get daemon version
    Version,

    /// Get daemon and GStreamer system info
    Sysinfo,

    /// Get pipeline count
    Count,

    /// List GStreamer elements
    Elements {
        /// Detail level (none, summary, full)
        #[arg(long, default_value = "none")]
        detail: String,
    },

    /// Discover media info for a URI
    Discover {
        /// Media URI
        uri: String,
        /// Discovery timeout in seconds
        #[arg(long)]
        timeout: Option<u32>,
    },

    /// Play a media URI using playbin
    PlayUri {
        /// Media URI
        uri: String,
        /// Use playbin2 instead of playbin3
        #[arg(long)]
        playbin2: bool,
    },
}

fn build_request(command: &Commands) -> Request {
    let id = uuid::Uuid::new_v4().to_string();

    match command {
        Commands::List => Request {
            id,
            method: "list_pipelines".to_string(),
            params: serde_json::json!({}),
        },
        Commands::Create { description } => Request {
            id,
            method: "create_pipeline".to_string(),
            params: serde_json::json!({ "description": description }),
        },
        Commands::Update {
            id: pid,
            description,
        } => Request {
            id,
            method: "update_pipeline".to_string(),
            params: serde_json::json!({ "pipeline_id": pid, "description": description }),
        },
        Commands::Remove { id: pid } => Request {
            id,
            method: "remove_pipeline".to_string(),
            params: serde_json::json!({ "pipeline_id": pid }),
        },
        Commands::Info { id: pid } => Request {
            id,
            method: "get_pipeline_info".to_string(),
            params: serde_json::json!({ "pipeline_id": pid }),
        },
        Commands::Play { id: pid } => Request {
            id,
            method: "play".to_string(),
            params: serde_json::json!({ "pipeline_id": pid }),
        },
        Commands::Pause { id: pid } => Request {
            id,
            method: "pause".to_string(),
            params: serde_json::json!({ "pipeline_id": pid }),
        },
        Commands::Stop { id: pid } => Request {
            id,
            method: "stop".to_string(),
            params: serde_json::json!({ "pipeline_id": pid }),
        },
        Commands::State { id: pid, state } => Request {
            id,
            method: "set_state".to_string(),
            params: serde_json::json!({ "pipeline_id": pid, "state": state }),
        },
        Commands::Snapshot { id: pid, details } => Request {
            id,
            method: "snapshot".to_string(),
            params: serde_json::json!({ "pipeline_id": pid, "details": details }),
        },
        Commands::Position { id: pid } => Request {
            id,
            method: "get_position".to_string(),
            params: serde_json::json!({ "pipeline_id": pid }),
        },
        Commands::Version => Request {
            id,
            method: "get_version".to_string(),
            params: serde_json::json!({}),
        },
        Commands::Sysinfo => Request {
            id,
            method: "get_info".to_string(),
            params: serde_json::json!({}),
        },
        Commands::Count => Request {
            id,
            method: "get_pipeline_count".to_string(),
            params: serde_json::json!({}),
        },
        Commands::Elements { detail } => Request {
            id,
            method: "get_elements".to_string(),
            params: serde_json::json!({ "detail": detail }),
        },
        Commands::Discover { uri, timeout } => Request {
            id,
            method: "discover_uri".to_string(),
            params: serde_json::json!({ "uri": uri, "timeout": timeout }),
        },
        Commands::PlayUri { uri, playbin2 } => Request {
            id,
            method: "play_uri".to_string(),
            params: serde_json::json!({ "uri": uri, "use_playbin2": playbin2 }),
        },
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let request = build_request(&cli.command);
    let request_id = request.id.clone();

    let (ws_stream, _) = match connect_async(&cli.url).await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Failed to connect to {}: {}", cli.url, e);
            std::process::exit(1);
        }
    };

    let (mut write, mut read) = ws_stream.split();

    let msg = match serde_json::to_string(&request) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to serialize request: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = write.send(Message::Text(msg.into())).await {
        eprintln!("Failed to send request: {}", e);
        std::process::exit(1);
    }

    // Wait for the response matching our request ID
    let timeout = tokio::time::Duration::from_secs(REQUEST_TIMEOUT_SECS);
    let result = tokio::time::timeout(timeout, async {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(response) = serde_json::from_str::<Response>(&text) {
                        let resp_id = match &response.id {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        if resp_id == request_id {
                            return Some(response);
                        }
                    }
                    // Ignore events and non-matching responses
                }
                Ok(Message::Close(_)) => {
                    eprintln!("Connection closed by server");
                    return None;
                }
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    return None;
                }
                _ => {}
            }
        }
        None
    })
    .await;

    // Send close frame before exiting
    let _ = write.send(Message::Close(None)).await;

    match result {
        Ok(Some(response)) => {
            if let Some(error) = response.error {
                eprintln!("Error (code {}): {}", error.code, error.message);
                std::process::exit(1);
            }
            if let Some(result) = response.result {
                serde_json::to_writer_pretty(std::io::stdout(), &result).unwrap();
                println!();
            }
        }
        Ok(None) => {
            eprintln!("No response received");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!(
                "Timeout: no response within {} seconds",
                REQUEST_TIMEOUT_SECS
            );
            std::process::exit(1);
        }
    }
}
