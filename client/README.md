# gstpop Clients

Interactive WebSocket clients for controlling gstpop. Both clients implement the same JSON-RPC 2.0 protocol and provide identical functionality.

## Available Clients

| Client | Language | Location | Dependencies |
|--------|----------|----------|--------------|
| gstpop-client | Rust | `rust/` | tokio, tungstenite, serde_json, rustyline |
| gstpop-client-c | C | `c/` | libsoup-2.4, json-glib, glib, readline |

## Features

- **Command history**: Use up/down arrow keys to navigate through previously entered commands (session-only, not persisted to disk)
- **Line editing**: Full readline-style editing with cursor movement, delete, backspace, etc.
- **JSON-RPC 2.0**: Standard protocol for communication with gstpop
- **Event handling**: Asynchronous display of pipeline events while maintaining input prompt

## Building

### Rust Client

```bash
cargo build --release -p gstpop-client
# Binary: target/release/gstpop-client
```

### C Client

```bash
meson setup builddir
ninja -C builddir gstpop-client-c
# Binary: builddir/client/c/gstpop-client-c
```

## Usage

```bash
# Connect to default address (ws://127.0.0.1:9000)
./gstpop-client

# Connect to custom address
./gstpop-client ws://192.168.1.100:8080
```

## Commands

| Command | Description | Example |
|---------|-------------|---------|
| `list` | List all pipelines | `list` |
| `create <desc>` | Create a new pipeline | `create videotestsrc ! autovideosink` |
| `update <id> <desc>` | Update pipeline description | `update 0 audiotestsrc ! autoaudiosink` |
| `remove <id>` | Remove a pipeline | `remove 0` |
| `info <id>` | Get pipeline info | `info 0` |
| `play [id]` | Play a pipeline | `play 0` |
| `pause [id]` | Pause a pipeline | `pause 0` |
| `stop [id]` | Stop a pipeline | `stop 0` |
| `state <id> <state>` | Set pipeline state | `state 0 playing` |
| `snapshot <id> [details]` | Get DOT graph | `snapshot 0 all` |
| `position [id]` | Get position/duration | `position 0` |
| `help` | Show available commands | `help` |
| `quit` | Exit the client | `quit` |

### State Values

For the `state` command: `null`, `ready`, `paused`, `playing`, `void_pending`

### DOT Graph Details

For the `snapshot` command: `media`, `caps`, `states`, `all` (optional, defaults to basic graph)

## Protocol

Both clients communicate with gstpop using JSON-RPC 2.0 over WebSocket.

### Request Format

```json
{
  "id": "unique-uuid",
  "method": "method_name",
  "params": { ... }
}
```

### Response Format

Success:
```json
{
  "jsonrpc": "2.0",
  "id": "unique-uuid",
  "result": { ... }
}
```

Error:
```json
{
  "jsonrpc": "2.0",
  "id": "unique-uuid",
  "error": {
    "code": -32000,
    "message": "Error description"
  }
}
```

### Events (Broadcast)

The daemon broadcasts events to all connected clients:

```json
{
  "event": "state_changed",
  "data": {
    "pipeline_id": "0",
    "old_state": "null",
    "new_state": "playing"
  }
}
```

Event types: `state_changed`, `error`, `eos`, `pipeline_added`, `pipeline_updated`, `pipeline_removed`

## Example Session

```
$ ./gstpop-client
Connecting to ws://127.0.0.1:9000...
Connected!

> list
[RESPONSE] id=...: {
  "pipelines": []
}

> create videotestsrc ! autovideosink
[RESPONSE] id=...: {
  "pipeline_id": "0"
}

> play 0
[RESPONSE] id=...: {
  "success": true
}

[EVENT] state_changed: {
  "pipeline_id": "0",
  "old_state": "null",
  "new_state": "playing"
}

> stop 0
> remove 0
> quit
Goodbye!
```
