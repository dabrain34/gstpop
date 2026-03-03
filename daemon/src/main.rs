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
#[command(about = "GStreamer Prince of Parser - Pipeline management daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the daemon with WebSocket and DBus interfaces
    Daemon(cmd::daemon::DaemonArgs),

    /// Launch pipelines and exit when all finish
    Launch(cmd::launch::LaunchArgs),

    /// Inspect GStreamer elements
    Inspect(cmd::inspect::InspectArgs),

    /// Discover media information for a URI
    Discover(cmd::discover::DiscoverArgs),

    /// Play a media URI using playbin
    Play(cmd::play::PlayArgs),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("gst_pop=info".parse().unwrap()),
        )
        .init();

    // Busybox-style binary name detection: if invoked as gst-pop-<subcommand>,
    // inject the subcommand into argv so clap routes to the right handler.
    let cli = {
        let mut args: Vec<String> = std::env::args().collect();
        let binary_name = args
            .first()
            .and_then(|s| {
                std::path::Path::new(s)
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
            })
            .unwrap_or_default();

        if binary_name == "gst-popd" {
            args[0] = "gst-pop".to_string();
            args.insert(1, "daemon".to_string());
        } else if let Some(suffix) = binary_name.strip_prefix("gst-pop-") {
            args[0] = "gst-pop".to_string();
            args.insert(1, suffix.to_string());
        }
        Cli::parse_from(args)
    };

    // Initialize GStreamer after CLI parsing so --help/--version are fast
    gstreamer::init().map_err(|e| {
        error!("Failed to initialize GStreamer: {}", e);
        e
    })?;

    let exit_code = match cli.command {
        Some(Commands::Daemon(args)) => cmd::daemon::run(args).await,
        Some(Commands::Launch(args)) => cmd::launch::run(args).await,
        Some(Commands::Inspect(args)) => cmd::inspect::run(args),
        Some(Commands::Discover(args)) => cmd::discover::run(args),
        Some(Commands::Play(args)) => cmd::play::run(args).await,
        None => {
            // No subcommand: default to daemon mode
            cmd::daemon::run(cmd::daemon::DaemonArgs::default()).await
        }
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

#[cfg(test)]
mod cli_tests;
