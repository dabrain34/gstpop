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
gstpop/
├── daemon/           # Rust server (WebSocket + DBus)
├── client/
│   ├── rust/         # Rust WebSocket client (gst-popctl)
│   └── c/            # C client
├── lib/              # C library (libgstpop)
├── web/              # Web frontend
├── docker/           # Dockerfile and docker-compose
├── scripts/          # Helper scripts
├── data/             # systemd service file
├── Formula/          # Homebrew formulae
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

## License

GstPrinceOfParser is distributed under the terms of the [GNU General Public License v3.0 or later](LICENSE).
See [LICENSE](LICENSE) for the full text.

## Credits

GstPrinceOfParser wouldn't exist without free and open-source software such as
GLib, GStreamer, and many more.

## Contributing

This project welcomes contributions, whether written by the contributor or generated with the assistance of AI tools.
What matters is that the human contributor fully understands every line of code they submit and can explain their reasoning when asked.
There is no rush to contribute—take the time needed to ensure your work is correct, well-tested, and complete.
New contributors must start with small, well-isolated changes accompanied by a clear explanation;
large changes from new contributors will be rejected regardless of how they were produced.
As trust is established through a track record of quality submissions, contributors may take on larger and more complex changes.
Contributions that appear to lack genuine understanding or create unnecessary review burden will be rejected without discussion.
Repeated low-quality submissions will result in a permanent ban.

You will need a [GitHub account](https://github.com/signup). [Fork](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/fork-a-repo) this repo, [clone](https://docs.github.com/en/repositories/creating-and-managing-repositories/cloning-a-repository) your fork, create a [feature branch](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository), [commit](http://git-scm.com/docs/git-commit), [push](http://git-scm.com/docs/git-push), and submit a [pull request](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-a-pull-request).

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

See [CHANGELOG.md](CHANGELOG.md) for the list of changes in each release.

### Creating a Release

Update the version in `Cargo.toml` (`[workspace.package]` section), run `cargo check` to update `Cargo.lock`, commit, and push the tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

This triggers the Release workflow which builds binaries for Linux, Windows, and macOS, creates `.deb` and `.rpm` packages, and publishes a GitHub Release with all artifacts.

### Docker

A multi-arch Docker image (amd64 + arm64) based on Fedora with all GStreamer plugins is published to GitHub Container Registry on each release.

#### Pull and run

```bash
docker pull ghcr.io/dabrain34/gstpop:latest

docker run -d -p 9000:9000 --name gst-pop ghcr.io/dabrain34/gstpop:latest
```

The daemon listens on port 9000 (WebSocket).

#### With docker compose

```bash
cd docker
docker compose up -d
```

#### Configuration

Pass environment variables to configure the daemon:

```bash
docker run -d -p 9000:9000 \
  -e GSTPOP_API_KEY=mysecretkey \
  -e RUST_LOG=gst_pop=debug \
  --name gst-pop ghcr.io/dabrain34/gstpop:latest
```

#### Verify

```bash
docker exec gst-pop gst-pop-inspect | wc -l
```

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
