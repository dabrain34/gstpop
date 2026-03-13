# Changelog

## [0.4.3] - 2026-03-13

- docker: add multi-arch Docker image (amd64 + arm64) to GHCR
- ci: add aarch64 RPM package build using native runner
- ci: add arm64 deb package build using native runner
- inspect: add gst-inspect-1.0 style terminal output
- launch: accept unquoted pipeline descriptions as positional arguments
- play: return exit code 69 for unsupported media and log the error
- websocket: fix clippy result_large_err with HandshakeValidator struct
