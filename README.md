### Description

**gpop** (GstPrinceOfParser) is a GStreamer pipeline management daemon that allows you to create, control, and monitor GStreamer media pipelines remotely via WebSocket or DBus interfaces.

### Why Use gpop?

#### Process Isolation
Run GStreamer pipelines in a separate process from your main application. If a pipeline crashes due to a buggy codec or driver issue, your application continues running unaffected.

#### Remote Pipeline Control
Control pipelines running on remote machines over the network. Perfect for:
- Headless media servers
- Distributed video processing
- IoT/embedded devices with limited UI

#### Multiple Pipeline Management
Create and manage multiple independent pipelines simultaneously:
- Run several video streams in parallel
- Mix and match different sources and sinks
- Monitor and control each pipeline individually

#### Language Agnostic
Any language that can speak WebSocket and JSON can control gpop:
- Use the provided Rust or C clients
- Integrate with Python, JavaScript, Go, or any other language
- Build custom dashboards or automation scripts

#### Real-time Monitoring
Receive live events for pipeline state changes, errors, and end-of-stream notifications via WebSocket broadcast.

### Project Structure

```
GstPrinceOfParser/
├── daemon/           # Rust server (WebSocket + DBus)
├── client/
│   ├── rust/         # Rust WebSocket client
│   └── c/            # C client
├── lib/              # C library (libgpop)
├── Cargo.toml        # Rust workspace
└── meson.build       # Build system (C + Rust)
```

### Dependencies

#### Linux (Debian/Ubuntu)

```bash
sudo apt install meson ninja-build rustc cargo \
  libglib2.0-dev libgstreamer1.0-dev \
  libsoup-3.0-dev libjson-glib-dev libreadline-dev
```

#### Linux (Fedora)

```bash
sudo dnf install meson ninja-build rust cargo \
  glib2-devel gstreamer1-devel \
  libsoup3-devel json-glib-devel readline-devel
```

### Build

```
meson setup builddir
ninja -C builddir
```

This builds everything:
- Rust daemon and client → `builddir/release/`
- C library → `builddir/lib/`

#### Build Options

| Option | Default | Description |
|--------|---------|-------------|
| `client` | `true` | Build the Rust client |
| `c_client` | `true` | Build the C client |

Example: build only the daemon (no clients):

```
meson setup builddir -Dclient=false -Dc_client=false
ninja -C builddir
```

### Usage

#### Running the Daemon

Start the WebSocket server:

```
./builddir/release/gpop-daemon
```

By default, the server binds to `ws://127.0.0.1:9000`.

Options:
- `--bind` / `-b`: IP address to bind to (default: `127.0.0.1`)
- `--port` / `-P`: Port to listen on (default: `9000`)
- `--pipeline` / `-p`: Initial pipeline(s) to create
- `--playback-mode` / `-x`: Auto-play all pipelines and exit when all reach EOS
- `--api-key`: API key for WebSocket authentication
- `--allowed-origin`: Allowed origins for WebSocket connections (CSRF protection)
- `--no-websocket`: Disable WebSocket interface
- `--no-dbus`: Disable DBus interface (Linux only)

Example with custom settings:

```
./builddir/release/gpop-daemon --bind 0.0.0.0 --port 8080
```

Example with authentication:

```
./builddir/release/gpop-daemon --api-key mysecretkey
# or via environment variable
GPOP_API_KEY=mysecretkey ./builddir/release/gpop-daemon
```

#### Running the Rust Client

```
./builddir/release/gpop-client
```

Or connect to a specific server:

```
./builddir/release/gpop-client ws://192.168.1.100:9000
```

See [daemon/README.md](daemon/README.md) for full API documentation.

### Security Considerations

#### Pipeline Descriptions

Pipeline descriptions are passed directly to GStreamer's `gst_parse_launch()`, allowing full GStreamer functionality. Authenticated clients can create pipelines that:

- Access local files (`filesrc`, `filesink`)
- Access network resources (`souphttpsrc`, `udpsrc`, `rtspsrc`)
- Use hardware devices (cameras, microphones, GPUs)

For security-sensitive deployments:
- Run the daemon with restricted filesystem/network permissions
- Use `--api-key` to require authentication
- Use `--allowed-origin` for browser-based clients (CSRF protection)

#### Authentication

- **API Key**: Use `--api-key` or `GPOP_API_KEY` environment variable
- **Origin Validation**: Use `--allowed-origin` to restrict browser origins

Note: Non-browser clients (CLI, scripts) don't send `Origin` headers and bypass origin validation when connecting directly.
