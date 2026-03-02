// main.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use clap::{Parser, Subcommand};
use tracing::error;
use tracing_subscriber::EnvFilter;

mod cmd;

#[derive(Parser, Debug)]
#[command(name = "gst-pop")]
#[command(author = "Stéphane Cerveau")]
#[command(version)]
#[command(about = "GStreamer Prince of Parser - Pipeline management tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the daemon with WebSocket and DBus interfaces
    Daemon(cmd::daemon::DaemonArgs),

    /// Play pipelines and exit when all finish
    Play(cmd::play::PlayArgs),

    /// Inspect GStreamer elements
    Inspect(cmd::inspect::InspectArgs),

    /// Discover media information for a URI
    Discover(cmd::discover::DiscoverArgs),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("gstpop=info".parse().unwrap()),
        )
        .init();

    // Initialize GStreamer once for all subcommands
    gstreamer::init().map_err(|e| {
        error!("Failed to initialize GStreamer: {}", e);
        e
    })?;

    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Daemon(args) => cmd::daemon::run(args).await,
        Commands::Play(args) => cmd::play::run(args).await,
        Commands::Inspect(args) => cmd::inspect::run(args),
        Commands::Discover(args) => cmd::discover::run(args),
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

#[cfg(test)]
mod cli_tests;
