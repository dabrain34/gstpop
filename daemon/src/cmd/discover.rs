// cmd/discover.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use clap::Args;
use tracing::error;

use gstpop::gst::discoverer;

/// Discover media information for a URI
#[derive(Args, Debug)]
pub struct DiscoverArgs {
    /// URI or file path to discover
    pub uri: String,

    /// Timeout in seconds for discovery
    #[arg(short, long, default_value_t = discoverer::DEFAULT_TIMEOUT_SECS)]
    pub timeout: u32,
}

pub fn run(args: DiscoverArgs) -> i32 {
    match discoverer::discover_uri(&args.uri, Some(args.timeout)) {
        Ok(info) => match serde_json::to_string_pretty(&info) {
            Ok(json) => {
                println!("{}", json);
                0
            }
            Err(e) => {
                error!("Failed to serialize discovery result: {}", e);
                1
            }
        },
        Err(e) => {
            error!("Discovery failed: {}", e);
            1
        }
    }
}
