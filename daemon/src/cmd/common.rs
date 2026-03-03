// cmd/common.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use clap::Args;

use gstpop::server::ServerConfig;

/// CLI flags for WebSocket and DBus server interfaces.
///
/// Shared across daemon, play, and launch subcommands.
#[derive(Args, Debug, Clone)]
pub struct ServerArgs {
    /// WebSocket port
    #[arg(short = 'P', long, default_value_t = gstpop::websocket::DEFAULT_WEBSOCKET_PORT)]
    pub port: u16,

    /// Bind address for WebSocket server
    #[arg(short, long, default_value = gstpop::websocket::DEFAULT_BIND_ADDRESS)]
    pub bind: String,

    /// Disable DBus interface (Linux only)
    #[cfg(target_os = "linux")]
    #[arg(long)]
    pub no_dbus: bool,

    /// Disable WebSocket interface
    #[arg(long)]
    pub no_websocket: bool,

    /// API key for WebSocket authentication (optional)
    #[arg(long, env = "GSTPOP_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    /// Allowed origins for WebSocket connections (optional, can be specified multiple times)
    /// If not specified, all origins are allowed. Use for CSRF protection in browser contexts.
    #[arg(long = "allowed-origin")]
    pub allowed_origins: Vec<String>,
}

impl Default for ServerArgs {
    fn default() -> Self {
        Self {
            port: gstpop::websocket::DEFAULT_WEBSOCKET_PORT,
            bind: gstpop::websocket::DEFAULT_BIND_ADDRESS.to_string(),
            #[cfg(target_os = "linux")]
            no_dbus: false,
            no_websocket: false,
            api_key: None,
            allowed_origins: Vec::new(),
        }
    }
}

impl ServerArgs {
    pub fn into_config(self) -> ServerConfig {
        ServerConfig {
            bind: self.bind,
            port: self.port,
            no_websocket: self.no_websocket,
            #[cfg(target_os = "linux")]
            no_dbus: self.no_dbus,
            #[cfg(not(target_os = "linux"))]
            no_dbus: true,
            api_key: self.api_key,
            allowed_origins: self.allowed_origins,
        }
    }
}
