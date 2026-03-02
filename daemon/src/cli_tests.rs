// cli_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use super::{Cli, Commands};
use clap::Parser;

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
}

#[test]
fn daemon_subcommand_parses() {
    let cli = Cli::parse_from(["gst-pop", "daemon"]);
    assert!(matches!(cli.command, Some(Commands::Daemon(_))));
}

#[test]
fn launch_subcommand_parses() {
    let cli = Cli::parse_from(["gst-pop", "launch", "-p", "fakesrc ! fakesink"]);
    assert!(matches!(cli.command, Some(Commands::Launch(_))));
}

#[test]
fn launch_positional_parses() {
    let cli = Cli::parse_from(["gst-pop", "launch", "fakesrc ! fakesink"]);
    if let Some(Commands::Launch(args)) = cli.command {
        assert_eq!(args.pipeline, Some("fakesrc ! fakesink".to_string()));
    } else {
        panic!("Expected Launch subcommand");
    }
}

#[test]
fn default_pipeline_positional() {
    let cli = Cli::parse_from(["gst-pop", "fakesrc", "!", "fakesink"]);
    assert!(cli.command.is_none());
    assert_eq!(cli.pipeline, vec!["fakesrc", "!", "fakesink"]);
}

#[test]
fn default_pipeline_quoted() {
    let cli = Cli::parse_from(["gst-pop", "fakesrc ! fakesink"]);
    assert!(cli.command.is_none());
    assert_eq!(cli.pipeline, vec!["fakesrc ! fakesink"]);
}

#[test]
fn no_args_gives_empty() {
    let cli = Cli::parse_from(["gst-pop"]);
    assert!(cli.command.is_none());
    assert!(cli.pipeline.is_empty());
}

#[test]
fn inspect_subcommand_parses() {
    let cli = Cli::parse_from(["gst-pop", "inspect"]);
    assert!(matches!(cli.command, Some(Commands::Inspect(_))));
}

#[test]
fn discover_subcommand_parses() {
    let cli = Cli::parse_from(["gst-pop", "discover", "file:///test.mp4"]);
    assert!(matches!(cli.command, Some(Commands::Discover(_))));
}

#[test]
fn play_subcommand_parses() {
    let cli = Cli::parse_from(["gst-pop", "play", "file:///test.mp4"]);
    assert!(matches!(cli.command, Some(Commands::Play(_))));
}

#[test]
fn play_with_sinks_parses() {
    let cli = Cli::parse_from([
        "gst-pop",
        "play",
        "file:///test.mp4",
        "--video-sink",
        "fakesink",
        "--audio-sink",
        "autoaudiosink",
    ]);
    if let Some(Commands::Play(args)) = cli.command {
        assert_eq!(args.uri, "file:///test.mp4");
        assert_eq!(args.video_sink, Some("fakesink".to_string()));
        assert_eq!(args.audio_sink, Some("autoaudiosink".to_string()));
        assert!(!args.playbin2);
    } else {
        panic!("Expected Play subcommand");
    }
}

#[test]
fn play_with_playbin2_parses() {
    let cli = Cli::parse_from(["gst-pop", "play", "file:///test.mp4", "--playbin2"]);
    if let Some(Commands::Play(args)) = cli.command {
        assert_eq!(args.uri, "file:///test.mp4");
        assert!(args.playbin2);
    } else {
        panic!("Expected Play subcommand");
    }
}
