### Description

**gst-pop** (GstPrinceOfParser) is a GStreamer pipeline management daemon that allows you to create, control, and monitor GStreamer media pipelines remotely via WebSocket or DBus interfaces.

### Why Use gst-pop?

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
Any language that can speak WebSocket and JSON can control gst-pop:
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
├── lib/              # C library (libgstpop)
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
| `cclient` | `false` | Build the C client |

Example: build only the daemon (no clients):

```
meson setup builddir -Dclient=false -Dcclient=false
ninja -C builddir
```

### Usage

#### Running the Daemon

Start the WebSocket server:

```
./builddir/release/gst-pop daemon
```

By default, the server binds to `ws://127.0.0.1:9000`.

#### Subcommands

| Subcommand | Description |
|------------|-------------|
| `daemon` | Start the WebSocket/DBus server |
| `launch` | Launch pipelines and exit when all finish (default subcommand) |
| `inspect` | Inspect GStreamer elements |
| `play` | Play a media URI using playbin3 (or playbin with `--playbin2`) |
| `discover` | Discover media information for a URI |

#### Busybox-style Symlinks (Unix)

On Unix systems, the installation creates symlinks that let you invoke subcommands directly:

| Symlink | Equivalent | Description |
|---------|------------|-------------|
| `gst-popd` | `gst-pop daemon` | Starts the daemon (used by the systemd service) |
| `gst-pop-launch` | `gst-pop launch` | Like `gst-launch-1.0` but with WebSocket and DBus interfaces |
| `gst-pop-inspect` | `gst-pop inspect` | Like `gst-inspect-1.0` |
| `gst-pop-discover` | `gst-pop discover` | Like `gst-discoverer-1.0` |
| `gst-pop-play` | `gst-pop play` | Like `gst-play-1.0` but with WebSocket and DBus interfaces |

#### Daemon Options
- `--bind` / `-b`: IP address to bind to (default: `127.0.0.1`)
- `--port` / `-P`: Port to listen on (default: `9000`)
- `--pipeline` / `-p`: Initial pipeline(s) to create
- `--api-key`: API key for WebSocket authentication
- `--allowed-origin`: Allowed origins for WebSocket connections (CSRF protection)
- `--no-websocket`: Disable WebSocket interface
- `--no-dbus`: Disable DBus interface (Linux only)

Example with custom settings:

```
./builddir/release/gst-pop daemon --bind 0.0.0.0 --port 8080
```

Example with authentication:

```
# Recommended: use environment variable (avoids exposing key in process listing)
GSTPOP_API_KEY=mysecretkey ./builddir/release/gst-pop daemon

# Alternative: via command-line argument (visible in `ps` output)
./builddir/release/gst-pop daemon --api-key mysecretkey
```

#### Running the Rust Client

```
./builddir/release/gst-popctl
```

Or connect to a specific server:

```
./builddir/release/gst-popctl ws://192.168.1.100:9000
```

See [daemon/README.md](daemon/README.md) for full API documentation.

### Creating a Release

Update the version in `Cargo.toml` (`[workspace.package]` section), run `cargo check` to update `Cargo.lock`, commit, and push the tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

This triggers the Release workflow which builds binaries for Linux, Windows, and macOS, creates `.deb` and `.rpm` packages, and publishes a GitHub Release with all artifacts.

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

- **API Key**: Use `--api-key` or `GSTPOP_API_KEY` environment variable
- **Origin Validation**: Use `--allowed-origin` to restrict browser origins

Note: Non-browser clients (CLI, scripts) don't send `Origin` headers and bypass origin validation when connecting directly.
