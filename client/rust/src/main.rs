// main.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use futures_util::{SinkExt, StreamExt};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

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

#[derive(Debug, Deserialize)]
struct Event {
    event: String,
    data: Value,
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn print_help() {
    println!("\nAvailable commands:");
    println!("  list                     - List all pipelines");
    println!("  create <description>     - Create a new pipeline");
    println!("  update <id> <description> - Update pipeline description");
    println!("  remove <id>              - Remove a pipeline");
    println!("  info <id>                - Get pipeline info");
    println!("  play [id]                - Play a pipeline");
    println!("  pause [id]               - Pause a pipeline");
    println!("  stop [id]                - Stop a pipeline");
    println!("  state <id> <state>       - Set pipeline state");
    println!("  snapshot <id> [details]  - Get DOT graph (details: media, caps, states, all)");
    println!("  position [id]            - Get pipeline position/duration (default: 0)");
    println!("  version                  - Get daemon version");
    println!("  sysinfo                  - Get daemon and GStreamer info");
    println!("  count                    - Get pipeline count");
    println!("  elements [detail]        - List GStreamer elements (detail: none, summary, full)");
    println!("  discover <uri> [timeout] - Discover media info for a URI");
    println!("  play_uri <uri> [playbin2] - Play a media URI using playbin3 (or playbin2)");
    println!("  help                     - Show this help");
    println!("  quit                     - Exit");
    println!();
}

fn parse_command(line: &str) -> Option<Request> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "list" => Some(Request {
            id: new_id(),
            method: "list_pipelines".to_string(),
            params: serde_json::json!({}),
        }),
        "create" if parts.len() > 1 => Some(Request {
            id: new_id(),
            method: "create_pipeline".to_string(),
            params: serde_json::json!({
                "description": parts[1..].join(" ")
            }),
        }),
        "update" if parts.len() > 2 => Some(Request {
            id: new_id(),
            method: "update_pipeline".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts[1],
                "description": parts[2..].join(" ")
            }),
        }),
        "remove" if parts.len() == 2 => Some(Request {
            id: new_id(),
            method: "remove_pipeline".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts[1]
            }),
        }),
        "info" if parts.len() == 2 => Some(Request {
            id: new_id(),
            method: "get_pipeline_info".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts[1]
            }),
        }),
        "play" => Some(Request {
            id: new_id(),
            method: "play".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts.get(1).copied()
            }),
        }),
        "pause" => Some(Request {
            id: new_id(),
            method: "pause".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts.get(1).copied()
            }),
        }),
        "stop" => Some(Request {
            id: new_id(),
            method: "stop".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts.get(1).copied()
            }),
        }),
        "state" if parts.len() == 3 => Some(Request {
            id: new_id(),
            method: "set_state".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts[1],
                "state": parts[2]
            }),
        }),
        "snapshot" if parts.len() >= 2 => Some(Request {
            id: new_id(),
            method: "snapshot".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts[1],
                "details": parts.get(2).copied()
            }),
        }),
        "position" => Some(Request {
            id: new_id(),
            method: "get_position".to_string(),
            params: serde_json::json!({
                "pipeline_id": parts.get(1).copied()
            }),
        }),
        "version" => Some(Request {
            id: new_id(),
            method: "get_version".to_string(),
            params: serde_json::json!({}),
        }),
        "sysinfo" => Some(Request {
            id: new_id(),
            method: "get_info".to_string(),
            params: serde_json::json!({}),
        }),
        "count" => Some(Request {
            id: new_id(),
            method: "get_pipeline_count".to_string(),
            params: serde_json::json!({}),
        }),
        "elements" => Some(Request {
            id: new_id(),
            method: "get_elements".to_string(),
            params: serde_json::json!({
                "detail": parts.get(1).copied()
            }),
        }),
        "discover" if parts.len() >= 2 => Some(Request {
            id: new_id(),
            method: "discover_uri".to_string(),
            params: serde_json::json!({
                "uri": parts[1],
                "timeout": parts.get(2).and_then(|s| s.parse::<u32>().ok())
            }),
        }),
        "play_uri" if parts.len() >= 2 => {
            let use_playbin2 = parts.get(2).map_or(false, |s| *s == "playbin2");
            Some(Request {
                id: new_id(),
                method: "play_uri".to_string(),
                params: serde_json::json!({
                    "uri": parts[1],
                    "use_playbin2": use_playbin2
                }),
            })
        }

        "help" => {
            print_help();
            None
        }
        _ => {
            println!("Unknown command or missing arguments. Type 'help' for available commands.");
            None
        }
    }
}

enum InputEvent {
    Line(String),
    Quit,
    Error(String),
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

    // Channel for sending commands from readline thread to async task
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<InputEvent>();

    // Spawn readline in a separate thread (rustyline is synchronous)
    let readline_handle = std::thread::spawn(move || {
        let mut rl = match DefaultEditor::new() {
            Ok(rl) => rl,
            Err(e) => {
                let _ = cmd_tx.send(InputEvent::Error(format!("Failed to create editor: {}", e)));
                return;
            }
        };

        loop {
            match rl.readline("> ") {
                Ok(line) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        let _ = rl.add_history_entry(trimmed);
                    }
                    if trimmed == "quit" || trimmed == "exit" {
                        let _ = cmd_tx.send(InputEvent::Quit);
                        break;
                    }
                    if cmd_tx.send(InputEvent::Line(line)).is_err() {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    let _ = cmd_tx.send(InputEvent::Quit);
                    break;
                }
                Err(ReadlineError::Eof) => {
                    let _ = cmd_tx.send(InputEvent::Quit);
                    break;
                }
                Err(e) => {
                    let _ = cmd_tx.send(InputEvent::Error(format!("Readline error: {}", e)));
                    break;
                }
            }
        }
    });

    // Spawn a task to read messages from WebSocket
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

    print_help();

    // Main command loop
    loop {
        tokio::select! {
            Some(event) = cmd_rx.recv() => {
                match event {
                    InputEvent::Line(line) => {
                        let trimmed = line.trim();
                        if let Some(request) = parse_command(trimmed) {
                            let msg = serde_json::to_string(&request)?;
                            println!("Sending: {}", msg);
                            write.send(Message::Text(msg.into())).await?;
                        }
                    }
                    InputEvent::Quit => {
                        break;
                    }
                    InputEvent::Error(e) => {
                        eprintln!("{}", e);
                        break;
                    }
                }
            }
            else => {
                break;
            }
        }
    }

    read_task.abort();
    let _ = readline_handle.join();
    println!("Goodbye!");
    Ok(())
}
