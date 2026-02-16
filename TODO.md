# TODO

## Features

- [x] Add message and error handling
- [ ] Handle case where the pipeline returns error which can be considered as not supported
- [ ] Add a way to handle GStreamer crash without crashing the daemon or save its context and reload it
- [ ] add seek
- [ ] add unique id to run multiple instance of the daemon
- [ ] get plugins and elements list
- [ ] get stats (buffers in srcs, sinks)
- [ ] Add command in argument such as launch, inspect, kill, etc.
- [ ] Add a logger
- [ ] add a scripting language such as capable of running multiple command


## Security

- [ ] Add GStreamer element allowlist/denylist for restricted deployments
- [ ] Add file-based secret loading as alternative to env/CLI API key
- [ ] Add `--strict-origin` flag to require Origin header for all clients
- [ ] Document deployment hardening (sandboxing, network namespacing)

## Code Quality

- [x] Add dropped event counter for monitoring slow WebSocket clients
- [ ] Consider extracting shared code between Rust client and daemon example
