# gst-pop

GStreamer Prince of Parser - A pipeline management tool with WebSocket and DBus interfaces.

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
  - [Source Layout](#source-layout)
  - [Component Diagram](#component-diagram)
  - [Key Design Decisions](#key-design-decisions)
- [Building](#building)
  - [With Cargo (standalone)](#with-cargo-standalone)
  - [With Meson (full project)](#with-meson-full-project)
- [Running](#running)
  - [Subcommands](#subcommands)
  - [Running the Server](#running-the-server)
  - [Daemon Options](#daemon-options)
  - [Environment Variables](#environment-variables)
- [Authentication](#authentication)
  - [Authentication Responses](#authentication-responses)
  - [Client Examples](#client-examples)
- [Launch Subcommand](#launch-subcommand)
  - [Exit Codes](#exit-codes)
- [Play Subcommand](#play-subcommand)
- [WebSocket API](#websocket-api)
  - [Protocol](#protocol)
  - [Methods](#methods)
  - [Events](#events)
  - [Error Codes](#error-codes)
- [Example Client](#example-client)
  - [Client Commands](#client-commands)
  - [Example Session](#example-session)
- [DBus Interface (Linux only)](#dbus-interface-linux-only)
  - [Manager Interface](#manager-interface)
  - [Pipeline Interface](#pipeline-interface)
  - [DBus Example](#dbus-example)
- [License](#license)

## Overview

`gst-pop` is a GStreamer pipeline management tool that allows you to create, control, and monitor GStreamer pipelines through WebSocket and DBus interfaces. It provides subcommands for running the daemon, launching pipelines, inspecting elements, and discovering media. Running a pipeline is the default action — `gst-pop "pipeline_desc"` works like `gst-launch-1.0`.

## Features

- **WebSocket API**: JSON-RPC 2.0 based protocol for pipeline management
- **DBus Interface** (Linux only): Native DBus integration for desktop applications
- **Real-time Events**: Receive pipeline state changes, errors, EOS, and lifecycle notifications
- **Pipeline Introspection**: Get DOT graph representations of pipelines

## Architecture

### Source Layout

```
daemon/src/
├── main.rs              # CLI entry point (clap), server startup, signal handling
├── lib.rs               # Public API re-exports
├── error.rs             # GstpopError enum (thiserror)
├── gst/
│   ├── mod.rs           # Module root, constants (MAX_PIPELINES, SHUTDOWN_GRACE_PERIOD_MS)
│   ├── event.rs         # PipelineEvent enum, PipelineState, broadcast channel factory
│   ├── manager.rs       # PipelineManager — thread-safe pipeline registry
│   └── pipeline.rs      # Pipeline — wraps gst::Pipeline, bus watcher, state control
├── websocket/
│   ├── mod.rs           # Module root, constants (MAX_CONCURRENT_CLIENTS, ports, buffers)
│   ├── server.rs        # WebSocketServer — TCP listener, auth, origin validation, event fan-out
│   ├── manager.rs       # ManagerInterface — routes JSON-RPC requests to PipelineManager
│   ├── pipeline.rs      # Request/result structs for pipeline operations
│   └── protocol.rs      # JSON-RPC 2.0 Request/Response/error code definitions
└── dbus/                # Linux only (gated with #[cfg(target_os = "linux")])
    ├── mod.rs           # DbusServer, event forwarder task
    ├── manager.rs       # org.gstpop.Manager interface (zbus)
    └── pipeline.rs      # org.gstpop.Pipeline interface (zbus)
```

### Component Diagram

```
                         ┌──────────────────────────────────┐
                         │            main.rs                │
                         │  CLI parsing, server bootstrap,   │
                         │  subcommand dispatch              │
                         └──────────┬───────────────────────┘
                                    │ creates
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
           ┌──────────────┐ ┌─────────────┐ ┌─────────────┐
           │ WebSocket    │ │   DBus      │ │  Playback   │
           │ Server       │ │  Server     │ │  Tracker    │
           │ (tokio task) │ │ (zbus,      │ │ (tokio task)│
           │              │ │  Linux only)│ │             │
           └──────┬───────┘ └──────┬──────┘ └──────┬──────┘
                  │                │               │
                  │  JSON-RPC      │  zbus methods  │ listens to
                  │  requests      │  & properties  │ events
                  ▼                ▼               │
           ┌─────────────────────────────┐        │
           │      PipelineManager        │        │
           │  Arc<RwLock<HashMap<        │        │
           │    String, Arc<Mutex<       │        │
           │      Pipeline>>>>>          │        │
           └──────────┬──────────────────┘        │
                      │ owns                      │
                      ▼                           │
           ┌─────────────────────┐                │
           │     Pipeline        │                │
           │  gst::Pipeline      │                │
           │  bus watcher task   │────────────────┘
           │  shutdown flag      │   sends events via
           └─────────────────────┘   broadcast channel
                      │
                      ▼
           ┌─────────────────────┐
           │  Event Broadcast    │
           │  Channel (256 buf)  │──► WebSocket clients
           │                     │──► DBus signal forwarder
           │                     │──► Playback tracker
           └─────────────────────┘
```

### Key Design Decisions

**Thread-safe pipeline management.** `PipelineManager` uses `RwLock<HashMap<String, Arc<Mutex<Pipeline>>>>`. The outer `RwLock` allows concurrent reads (listing, querying) while serializing writes (add, remove, update). Each `Pipeline` has its own `Mutex` so operations on different pipelines don't block each other.

**Event broadcasting.** A single `tokio::sync::broadcast` channel (capacity 256) distributes `PipelineEvent`s to all subscribers. Each bus watcher task sends events into the channel; the WebSocket server, DBus forwarder, and playback tracker each hold their own receiver. This decouples producers from consumers — adding a new consumer requires only calling `event_tx.subscribe()`.

**Bus watcher per pipeline.** Each `Pipeline` spawns a tokio task that polls the GStreamer bus via `spawn_blocking` (100ms timeout per poll) to avoid blocking the async runtime. The task checks an `AtomicBool` shutdown flag between polls for clean teardown.

**JSON-RPC 2.0 over WebSocket.** The WebSocket server accepts TCP connections, performs the HTTP upgrade with optional API key authentication and origin validation, then spawns two tasks per client: one reads incoming JSON-RPC requests and routes them through `ManagerInterface`, the other forwards broadcast events. Per-client message channels are bounded (`CLIENT_MESSAGE_BUFFER = 256`) to apply backpressure to slow clients.

**Platform-specific DBus.** The entire `dbus/` module is conditionally compiled with `#[cfg(target_os = "linux")]`. The `DbusServer` listens for `PipelineAdded`/`PipelineRemoved` events and dynamically registers/unregisters `org.gstpop.Pipeline{N}` objects on the session bus via zbus.

**Launch subcommand (default).** The `gst-pop launch` subcommand (also the default when a pipeline description is given directly) runs a dedicated tokio task that tracks pipeline completion events against a `HashSet<String>` of pending pipeline IDs. When all pipelines finish, a oneshot channel signals the main loop to exit with the appropriate exit code (0 for success, 1 for error, 69 for unsupported media).

## Building

### With Cargo (standalone)

```bash
cd daemon
cargo build --release
```

The binary will be at `target/release/gst-pop`.

### With Meson (full project)

From the project root:

```bash
meson setup builddir
ninja -C builddir
```

The binary will be at `builddir/release/gst-pop`.

To build only the daemon (without clients):

```bash
meson setup builddir -Dclient=false -Dc_client=false
ninja -C builddir
```

## Running

`gst-pop` uses subcommands. Run `gst-pop --help` or `gst-pop <subcommand> --help` for full details.

### Subcommands

| Subcommand | Description |
|------------|-------------|
| `gst-pop daemon` | Start the WebSocket/DBus server |
| `gst-pop launch` | Launch pipelines and exit when all finish |
| `gst-pop inspect` | Inspect GStreamer elements |
| `gst-pop play` | Play a media URI using playbin3 (or playbin with `--playbin2`) |
| `gst-pop discover` | Discover media information for a URI |
| `gst-pop <PIPELINE>` | Default: launch a single pipeline (same as `gst-pop launch <PIPELINE>`) |

### Running the Server

```bash
# Default: bind to 127.0.0.1:9000
gst-pop daemon

# Custom bind address and port
gst-pop daemon --bind 0.0.0.0 --port 8080

# With initial pipeline
gst-pop daemon -p "videotestsrc ! autovideosink"

# With authentication
gst-pop daemon --api-key mysecretkey

# Enable debug logging
RUST_LOG=debug gst-pop daemon
```

### Daemon Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--bind` | `-b` | `127.0.0.1` | IP address to bind to |
| `--port` | `-P` | `9000` | Port to listen on |
| `--pipeline` | `-p` | - | Initial pipeline(s) to create (can be repeated) |
| `--api-key` | - | - | API key for WebSocket authentication |
| `--allowed-origin` | - | - | Allowed origins for WebSocket connections (can be repeated) |
| `--no-websocket` | - | - | Disable WebSocket interface |
| `--no-dbus` | - | - | Disable DBus interface (Linux only) |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `GSTPOP_API_KEY` | API key for WebSocket authentication (alternative to `--api-key`) |
| `RUST_LOG` | Log level (e.g., `debug`, `info`, `warn`, `error`) |

## Authentication

By default, the WebSocket server accepts connections from any client that can reach it. When binding to `127.0.0.1` (the default), only local processes can connect.

For deployments where the server is exposed on a network or in multi-user environments, you can enable API key authentication:

```bash
# Via command line
gst-pop daemon --api-key mysecretkey

# Via environment variable
GSTPOP_API_KEY=mysecretkey gst-pop daemon

# Combined with network binding
gst-pop daemon --bind 0.0.0.0 --api-key mysecretkey
```

When authentication is enabled, clients must include the API key in the `Authorization` header during the WebSocket handshake:

```
Authorization: mysecretkey
```

### Authentication Responses

| Scenario | HTTP Status |
|----------|-------------|
| No `--api-key` configured | Connection accepted (no auth required) |
| Correct API key provided | Connection accepted |
| Missing `Authorization` header | `401 Unauthorized` |
| Invalid API key | `403 Forbidden` |

### Client Examples

**JavaScript (Browser/Node.js):**
```javascript
const ws = new WebSocket('ws://localhost:9000', {
  headers: {
    'Authorization': 'mysecretkey'
  }
});
```

**Python (websockets library):**
```python
import websockets

async def connect():
    async with websockets.connect(
        'ws://localhost:9000',
        extra_headers={'Authorization': 'mysecretkey'}
    ) as ws:
        await ws.send('{"id":"1","method":"list_pipelines","params":{}}')
        print(await ws.recv())
```

**Rust (tokio-tungstenite):**
```rust
use tokio_tungstenite::tungstenite::http::Request;

let request = Request::builder()
    .uri("ws://localhost:9000")
    .header("Authorization", "mysecretkey")
    .body(())?;

let (ws_stream, _) = connect_async(request).await?;
```

**curl (for testing handshake):**
```bash
# This will fail with 401 if auth is enabled and no key provided
curl -i -N \
  -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  -H "Sec-WebSocket-Version: 13" \
  -H "Authorization: mysecretkey" \
  http://localhost:9000/
```

## Launch Subcommand

The `launch` subcommand (also the default) turns gst-pop into a batch pipeline runner. Pipelines are automatically played on startup and gst-pop exits when every pipeline has finished.

```bash
# Launch a single pipeline (default subcommand, works like gst-launch-1.0)
gst-pop "filesrc location=video.mp4 ! decodebin ! fakesink"

# Equivalent explicit form
gst-pop launch "filesrc location=video.mp4 ! decodebin ! fakesink"

# Launch multiple pipelines in parallel with -p flags, exit when all finish
gst-pop launch \
  -p "filesrc location=video1.mp4 ! decodebin ! fakesink" \
  -p "filesrc location=video2.mp4 ! decodebin ! fakesink"
```

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All pipelines reached EOS successfully |
| `1` | At least one pipeline errored |
| `69` | At least one pipeline had unsupported media (EX_UNAVAILABLE, matching gst-launch convention) |

Error takes priority over unsupported: if any pipeline errors and another has unsupported media, exit code is `1`.

## Play Subcommand

The `play` subcommand plays a media URI using GStreamer's `playbin3` (default) or legacy `playbin`. It accepts file paths, `file://` URIs, and network URIs (`http://`, `rtsp://`, etc.).

```bash
# Play a local file (bare path is automatically converted to file:// URI)
gst-pop play /path/to/video.mp4

# Play a network stream
gst-pop play https://example.com/stream.mp4

# Override the video or audio sink
gst-pop play video.mp4 --video-sink fakesink --audio-sink autoaudiosink

# Use legacy playbin instead of playbin3
gst-pop play video.mp4 --playbin2
```

The same functionality is available via WebSocket (`play_uri` method) and DBus (`PlayUri` method) when the daemon is running.

## WebSocket API

Connect to `ws://<host>:<port>` to interact with the server.

### Protocol

All messages use JSON-RPC 2.0 format:

**Request:**
```json
{
  "id": "unique-request-id",
  "method": "method_name",
  "params": { ... }
}
```

**Success Response:**
```json
{
  "id": "unique-request-id",
  "result": { ... }
}
```

**Error Response:**
```json
{
  "id": "unique-request-id",
  "error": {
    "code": -32000,
    "message": "Error description"
  }
}
```

### Methods

#### `list_pipelines`

List all managed pipelines.

**Request:**
```json
{
  "id": "1",
  "method": "list_pipelines",
  "params": {}
}
```

**Response:**
```json
{
  "id": "1",
  "result": {
    "pipelines": [
      {
        "id": "0",
        "description": "videotestsrc ! autovideosink",
        "state": "playing",
        "streaming": true
      }
    ]
  }
}
```

#### `create_pipeline`

Create a new pipeline from a GStreamer pipeline description.

**Request:**
```json
{
  "id": "2",
  "method": "create_pipeline",
  "params": {
    "description": "videotestsrc ! autovideosink"
  }
}
```

**Response:**
```json
{
  "id": "2",
  "result": {
    "pipeline_id": "0"
  }
}
```

#### `remove_pipeline`

Remove and destroy a pipeline.

**Request:**
```json
{
  "id": "3",
  "method": "remove_pipeline",
  "params": {
    "pipeline_id": "0"
  }
}
```

**Response:**
```json
{
  "id": "3",
  "result": {}
}
```

#### `get_pipeline_info`

Get information about a specific pipeline.

**Request:**
```json
{
  "id": "4",
  "method": "get_pipeline_info",
  "params": {
    "pipeline_id": "0"
  }
}
```

**Response:**
```json
{
  "id": "4",
  "result": {
    "id": "0",
    "description": "videotestsrc ! autovideosink",
    "state": "playing",
    "streaming": true
  }
}
```

#### `set_state`

Set the pipeline state.

**Request:**
```json
{
  "id": "5",
  "method": "set_state",
  "params": {
    "pipeline_id": "0",
    "state": "playing"
  }
}
```

Valid states: `null`, `ready`, `paused`, `playing`

**Response:**
```json
{
  "id": "5",
  "result": {
    "success": true
  }
}
```

#### `play`, `pause`, `stop`

Convenience methods for state changes. The `pipeline_id` parameter is optional and defaults to `"0"`.

**Request:**
```json
{
  "id": "6",
  "method": "play",
  "params": {}
}
```

Or with explicit pipeline ID:
```json
{
  "id": "6",
  "method": "play",
  "params": {
    "pipeline_id": "0"
  }
}
```

#### `update_pipeline`

Update an existing pipeline with a new description. The pipeline is stopped and replaced atomically.

**Request:**
```json
{
  "id": "7",
  "method": "update_pipeline",
  "params": {
    "pipeline_id": "0",
    "description": "videotestsrc pattern=ball ! autovideosink"
  }
}
```

**Response:**
```json
{
  "id": "7",
  "result": {
    "success": true
  }
}
```

#### `get_position`

Get the current position and duration of a pipeline. The `pipeline_id` parameter is optional and defaults to `"0"`.

**Request:**
```json
{
  "id": "8",
  "method": "get_position",
  "params": {}
}
```

**Response:**
```json
{
  "id": "8",
  "result": {
    "position_ns": 1500000000,
    "duration_ns": 10000000000,
    "progress": 0.15
  }
}
```

Note: `position_ns` and `duration_ns` are in nanoseconds. `progress` is a value between 0.0 and 1.0. Any of these fields may be `null` if not available (e.g., for live streams).

#### `get_version`

Get the daemon version.

**Request:**
```json
{
  "id": "9",
  "method": "get_version",
  "params": {}
}
```

**Response:**
```json
{
  "id": "9",
  "result": {
    "version": "0.2.0"
  }
}
```

#### `get_info`

Get daemon and GStreamer version information.

**Request:**
```json
{
  "id": "10",
  "method": "get_info",
  "params": {}
}
```

**Response:**
```json
{
  "id": "10",
  "result": {
    "daemon_version": "0.2.0",
    "gstreamer_version": "GStreamer 1.24.0",
    "jsonrpc_version": "2.0"
  }
}
```

#### `get_pipeline_count`

Get the number of managed pipelines.

**Request:**
```json
{
  "id": "10",
  "method": "get_pipeline_count",
  "params": {}
}
```

**Response:**
```json
{
  "id": "10",
  "result": {
    "count": 3
  }
}
```

#### `snapshot`

Get the DOT graph representation of a pipeline. The `pipeline_id` parameter is optional and defaults to `"0"`.

**Request:**
```json
{
  "id": "7",
  "method": "snapshot",
  "params": {}
}
```

Or with explicit pipeline ID and detail level:
```json
{
  "id": "7",
  "method": "snapshot",
  "params": {
    "pipeline_id": "0",
    "details": "all"
  }
}
```

Valid detail levels: `media`, `caps`, `non-default`, `states`, `all` (default)

**Response:**
```json
{
  "id": "7",
  "result": {
    "dot": "digraph pipeline { ... }"
  }
}
```

### Events

The server broadcasts events to all connected clients:

#### `state_changed`
```json
{
  "event": "state_changed",
  "data": {
    "pipeline_id": "0",
    "old_state": "paused",
    "new_state": "playing"
  }
}
```

#### `error`
```json
{
  "event": "error",
  "data": {
    "pipeline_id": "0",
    "message": "Error description"
  }
}
```

#### `unsupported`

Emitted when a pipeline fails due to missing codec, unsupported format, or hardware limitation.

```json
{
  "event": "unsupported",
  "data": {
    "pipeline_id": "0",
    "message": "No decoder available for type 'video/x-h265'"
  }
}
```

#### `eos`
```json
{
  "event": "eos",
  "data": {
    "pipeline_id": "0"
  }
}
```

#### `pipeline_added`
```json
{
  "event": "pipeline_added",
  "data": {
    "pipeline_id": "0",
    "description": "videotestsrc ! autovideosink"
  }
}
```

#### `pipeline_updated`
```json
{
  "event": "pipeline_updated",
  "data": {
    "pipeline_id": "0",
    "description": "videotestsrc ! autovideosink"
  }
}
```

#### `pipeline_removed`
```json
{
  "event": "pipeline_removed",
  "data": {
    "pipeline_id": "0"
  }
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `-32700` | Parse error - Invalid JSON |
| `-32601` | Method not found |
| `-32602` | Invalid params |
| `-32603` | Internal error |
| `-32000` | Pipeline not found |
| `-32001` | Pipeline creation failed |
| `-32002` | State change failed |
| `-32003` | GStreamer error |
| `-32005` | Media not supported (missing codec, unsupported format, hardware limitation) |

## Example Client

An interactive WebSocket client is included:

```bash
cargo run --example ws_client

# Or connect to a different server
cargo run --example ws_client -- ws://192.168.1.100:9000
```

### Client Commands

```
list                        - List all pipelines
create <description>        - Create a new pipeline
update <id> <description>   - Update pipeline description
remove <id>                 - Remove a pipeline
info <id>                   - Get pipeline info
play [id]                   - Play a pipeline (default: 0)
pause [id]                  - Pause a pipeline (default: 0)
stop [id]                   - Stop a pipeline (default: 0)
state <id> <state>          - Set pipeline state
position [id]               - Get pipeline position/duration (default: 0)
snapshot [id] [details]     - Get DOT graph (default: 0)
version                     - Get daemon version
sysinfo                     - Get daemon and GStreamer info
count                       - Get pipeline count
quit                        - Exit
```

### Example Session

```
$ cargo run --example ws_client
Connecting to ws://127.0.0.1:9000...
Connected!

> create videotestsrc ! autovideosink
Sending: {"id":"...","method":"create_pipeline","params":{"description":"videotestsrc ! autovideosink"}}

[RESPONSE] id=...: {
  "pipeline_id": "0"
}

> play
Sending: {"id":"...","method":"play","params":{}}

[EVENT] state_changed: {"new_state":"ready","old_state":"null","pipeline_id":"0"}
[EVENT] state_changed: {"new_state":"paused","old_state":"ready","pipeline_id":"0"}
[RESPONSE] id=...: {
  "success": true
}
[EVENT] state_changed: {"new_state":"playing","old_state":"paused","pipeline_id":"0"}

> snapshot
Sending: {"id":"...","method":"snapshot","params":{}}

[RESPONSE] id=...: {
  "dot": "digraph pipeline { ... }"
}

> list
Sending: {"id":"...","method":"list_pipelines","params":{}}

[RESPONSE] id=...: {
  "pipelines": [
    {
      "description": "videotestsrc ! autovideosink",
      "id": "0",
      "state": "playing",
      "streaming": true
    }
  ]
}

> stop
> remove 0
> quit
Goodbye!
```

## DBus Interface (Linux only)

On Linux, gst-pop also exposes a DBus interface on the session bus.

### Service Name

`org.gstpop`

### Manager Interface

**Path:** `/org/gstpop/Manager`
**Interface:** `org.gstpop.Manager`

#### Methods

- `AddPipeline(description: string) -> string` - Create a pipeline, returns ID
- `RemovePipeline(id: string)` - Remove a pipeline
- `GetPipelineDesc(id: string) -> string` - Get pipeline description
- `UpdatePipeline(id: string, description: string)` - Update pipeline description

#### Properties

- `Pipelines: u32` - Number of active pipelines
- `Version: string` - Daemon version
- `GStreamerVersion: string` - GStreamer version string (e.g., "GStreamer 1.24.0")

#### Signals

- `PipelineAdded(id: string, description: string)`
- `PipelineRemoved(id: string)`

### Pipeline Interface

**Path:** `/org/gstpop/Pipeline{N}` (e.g., `/org/gstpop/Pipeline0`)
**Interface:** `org.gstpop.Pipeline`

#### Methods

- `SetState(state: string) -> bool` - Set pipeline state
- `Play() -> bool` - Start playing
- `Pause() -> bool` - Pause playback
- `Stop() -> bool` - Stop pipeline
- `GetDot(details: string) -> string` - Get DOT graph (details: "media", "caps", "non-default", "states", "all", or empty for all)
- `GetPosition() -> (i64, i64)` - Get (position_ns, duration_ns), -1 if unavailable
- `Update(description: string) -> bool` - Update pipeline with new description

#### Properties

- `Id: string` - Pipeline ID
- `Description: string` - Pipeline description
- `State: string` - Current state
- `Streaming: bool` - Whether pipeline is streaming

#### Signals

- `StateChanged(old_state: string, new_state: string)`
- `Error(message: string)`
- `Eos()`

### DBus Example

```bash
# List pipelines count
dbus-send --session --print-reply --dest=org.gstpop \
  /org/gstpop/Manager org.freedesktop.DBus.Properties.Get \
  string:org.gstpop.Manager string:Pipelines

# Get GStreamer version
dbus-send --session --print-reply --dest=org.gstpop \
  /org/gstpop/Manager org.freedesktop.DBus.Properties.Get \
  string:org.gstpop.Manager string:GStreamerVersion

# Create a pipeline
dbus-send --session --print-reply --dest=org.gstpop \
  /org/gstpop/Manager org.gstpop.Manager.AddPipeline \
  string:"videotestsrc ! fakesink"

# Play a pipeline
dbus-send --session --print-reply --dest=org.gstpop \
  /org/gstpop/Pipeline0 org.gstpop.Pipeline.Play

# Get DOT graph (all details)
dbus-send --session --print-reply --dest=org.gstpop \
  /org/gstpop/Pipeline0 org.gstpop.Pipeline.GetDot \
  string:""

# Get position/duration
dbus-send --session --print-reply --dest=org.gstpop \
  /org/gstpop/Pipeline0 org.gstpop.Pipeline.GetPosition

# Update pipeline
dbus-send --session --print-reply --dest=org.gstpop \
  /org/gstpop/Pipeline0 org.gstpop.Pipeline.Update \
  string:"videotestsrc pattern=ball ! fakesink"
```

## License

GPL-3.0-only
