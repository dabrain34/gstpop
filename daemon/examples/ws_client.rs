// ws_client.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Serialize)]
struct Request {
    id: String,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct Response {
    id: String,
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

#[derive(Debug, Deserialize)]
struct Event {
    event: String,
    data: Value,
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ws://127.0.0.1:9000".to_string());

    println!("Connecting to {}...", url);

    let (ws_stream, _) = connect_async(&url).await?;
    println!("Connected!");

    let (mut write, mut read) = ws_stream.split();

    // Spawn a task to read messages
    let read_task = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Try to parse as event first
                    if let Ok(event) = serde_json::from_str::<Event>(&text) {
                        println!("\n[EVENT] {}: {:?}", event.event, event.data);
                    } else if let Ok(response) = serde_json::from_str::<Response>(&text) {
                        if let Some(error) = response.error {
                            println!(
                                "\n[ERROR] id={}: {} (code: {})",
                                response.id, error.message, error.code
                            );
                        } else if let Some(result) = response.result {
                            println!(
                                "\n[RESPONSE] id={}: {}",
                                response.id,
                                serde_json::to_string_pretty(&result).unwrap()
                            );
                        }
                    } else {
                        println!("\n[RAW] {}", text);
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("\nConnection closed");
                    break;
                }
                Err(e) => {
                    eprintln!("\nError: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Interactive command loop
    println!("\nAvailable commands:");
    println!("  list                        - List all pipelines");
    println!("  create <description>        - Create a new pipeline");
    println!("  update <id> <description>   - Update pipeline description");
    println!("  remove <id>                 - Remove a pipeline");
    println!("  info <id>                   - Get pipeline info");
    println!("  play [id]                   - Play a pipeline (default: 0)");
    println!("  pause [id]                  - Pause a pipeline (default: 0)");
    println!("  stop [id]                   - Stop a pipeline (default: 0)");
    println!("  state <id> <state>          - Set pipeline state");
    println!("  position [id]               - Get pipeline position/duration (default: 0)");
    println!("  snapshot [id] [details]     - Get DOT graph (default: 0)");
    println!("  version                     - Get daemon version");
    println!("  sysinfo                     - Get daemon and GStreamer info");
    println!("  count                       - Get pipeline count");
    println!(
        "  elements [detail]           - List GStreamer elements (detail: none, summary, full)"
    );
    println!("  discover <uri> [timeout]    - Discover media info for a URI");
    println!("  quit                        - Exit");
    println!();

    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);

    loop {
        use tokio::io::AsyncBufReadExt;

        print!("> ");
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let request = match parts[0] {
            "list" => Request {
                id: new_id(),
                method: "list_pipelines".to_string(),
                params: serde_json::json!({}),
            },
            "create" if parts.len() > 1 => Request {
                id: new_id(),
                method: "create_pipeline".to_string(),
                params: serde_json::json!({
                    "description": parts[1..].join(" ")
                }),
            },
            "update" if parts.len() > 2 => Request {
                id: new_id(),
                method: "update_pipeline".to_string(),
                params: serde_json::json!({
                    "pipeline_id": parts[1],
                    "description": parts[2..].join(" ")
                }),
            },
            "remove" if parts.len() == 2 => Request {
                id: new_id(),
                method: "remove_pipeline".to_string(),
                params: serde_json::json!({
                    "pipeline_id": parts[1]
                }),
            },
            "info" if parts.len() == 2 => Request {
                id: new_id(),
                method: "get_pipeline_info".to_string(),
                params: serde_json::json!({
                    "pipeline_id": parts[1]
                }),
            },
            "play" => Request {
                id: new_id(),
                method: "play".to_string(),
                params: if parts.len() >= 2 {
                    serde_json::json!({ "pipeline_id": parts[1] })
                } else {
                    serde_json::json!({})
                },
            },
            "pause" => Request {
                id: new_id(),
                method: "pause".to_string(),
                params: if parts.len() >= 2 {
                    serde_json::json!({ "pipeline_id": parts[1] })
                } else {
                    serde_json::json!({})
                },
            },
            "stop" => Request {
                id: new_id(),
                method: "stop".to_string(),
                params: if parts.len() >= 2 {
                    serde_json::json!({ "pipeline_id": parts[1] })
                } else {
                    serde_json::json!({})
                },
            },
            "state" if parts.len() == 3 => Request {
                id: new_id(),
                method: "set_state".to_string(),
                params: serde_json::json!({
                    "pipeline_id": parts[1],
                    "state": parts[2]
                }),
            },
            "snapshot" => Request {
                id: new_id(),
                method: "snapshot".to_string(),
                params: if parts.len() >= 3 {
                    serde_json::json!({
                        "pipeline_id": parts[1],
                        "details": parts[2]
                    })
                } else if parts.len() == 2 {
                    serde_json::json!({ "pipeline_id": parts[1] })
                } else {
                    serde_json::json!({})
                },
            },
            "position" => Request {
                id: new_id(),
                method: "get_position".to_string(),
                params: if parts.len() >= 2 {
                    serde_json::json!({ "pipeline_id": parts[1] })
                } else {
                    serde_json::json!({})
                },
            },
            "version" => Request {
                id: new_id(),
                method: "get_version".to_string(),
                params: serde_json::json!({}),
            },
            "sysinfo" => Request {
                id: new_id(),
                method: "get_info".to_string(),
                params: serde_json::json!({}),
            },
            "count" => Request {
                id: new_id(),
                method: "get_pipeline_count".to_string(),
                params: serde_json::json!({}),
            },
            "elements" => Request {
                id: new_id(),
                method: "get_elements".to_string(),
                params: if parts.len() >= 2 {
                    serde_json::json!({ "detail": parts[1] })
                } else {
                    serde_json::json!({})
                },
            },
            "discover" if parts.len() >= 2 => Request {
                id: new_id(),
                method: "discover_uri".to_string(),
                params: serde_json::json!({
                    "uri": parts[1],
                    "timeout": parts.get(2).and_then(|s| s.parse::<u32>().ok())
                }),
            },
            "quit" | "exit" => {
                break;
            }
            _ => {
                println!("Unknown command or missing arguments");
                continue;
            }
        };

        let msg = serde_json::to_string(&request)?;
        println!("Sending: {}", msg);
        write.send(Message::Text(msg.into())).await?;
    }

    read_task.abort();
    println!("Goodbye!");
    Ok(())
}
