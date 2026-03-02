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
    command: Option<Commands>,

    /// Pipeline description to launch (default command)
    #[arg(trailing_var_arg = true, value_name = "PIPELINE")]
    pipeline: Vec<String>,
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
        Some(Commands::Daemon(args)) => cmd::daemon::run(args).await,
        Some(Commands::Launch(args)) => cmd::launch::run(args).await,
        Some(Commands::Inspect(args)) => cmd::inspect::run(args),
        Some(Commands::Discover(args)) => cmd::discover::run(args),
        Some(Commands::Play(args)) => cmd::play::run(args).await,
        None => {
            if cli.pipeline.is_empty() {
                // No subcommand and no pipeline: print help
                use clap::CommandFactory;
                Cli::command().print_help().unwrap();
                println!();
                0
            } else {
                let pipeline = cli.pipeline.join(" ");
                let args = cmd::launch::LaunchArgs {
                    pipelines: vec![],
                    pipeline: Some(pipeline),
                };
                cmd::launch::run(args).await
            }
        }
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

#[cfg(test)]
mod cli_tests;
