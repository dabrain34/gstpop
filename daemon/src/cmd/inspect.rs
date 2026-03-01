// cmd/inspect.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use clap::Args;
use tracing::error;

use gstpop::gst::registry::{self, DetailLevel};

/// Inspect GStreamer elements
#[derive(Args, Debug)]
pub struct InspectArgs {
    /// Element name to inspect (omit to list all elements)
    pub element: Option<String>,

    /// Detail level: none, summary, full
    #[arg(short, long, default_value = "summary")]
    pub detail: String,
}

pub fn run(args: InspectArgs) -> i32 {
    let detail = match args.detail.parse::<DetailLevel>() {
        Ok(d) => d,
        Err(e) => {
            error!("{}", e);
            return 1;
        }
    };

    if let Some(name) = &args.element {
        match registry::get_element(name, detail) {
            Some(info) => match serde_json::to_string_pretty(&info) {
                Ok(json) => {
                    println!("{}", json);
                    0
                }
                Err(e) => {
                    error!("Failed to serialize element info: {}", e);
                    1
                }
            },
            None => {
                error!("Element '{}' not found", name);
                1
            }
        }
    } else {
        let elements = registry::get_elements(detail);
        match serde_json::to_string_pretty(&elements) {
            Ok(json) => {
                println!("{}", json);
                0
            }
            Err(e) => {
                error!("Failed to serialize elements: {}", e);
                1
            }
        }
    }
}
