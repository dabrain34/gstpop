// cmd/inspect.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use clap::Args;
use tracing::error;

use gstpop::gst::inspect_format;
use gstpop::gst::registry::{self, DetailLevel};

/// Inspect GStreamer elements
#[derive(Args, Debug)]
pub struct InspectArgs {
    /// Element name to inspect (omit to list all elements)
    pub element: Option<String>,

    /// Detail level: none, summary, full
    #[arg(short, long, default_value = "summary")]
    pub detail: String,

    /// Output format: text or json
    #[arg(short, long, default_value = "text")]
    pub format: String,
}

pub fn run(args: InspectArgs) -> i32 {
    let detail = match args.detail.parse::<DetailLevel>() {
        Ok(d) => d,
        Err(e) => {
            error!("{}", e);
            return 1;
        }
    };

    let use_json = match args.format.as_str() {
        "json" => true,
        "text" => false,
        other => {
            error!("Invalid format: '{}'. Expected 'text' or 'json'", other);
            return 1;
        }
    };

    if let Some(name) = &args.element {
        // For text output, force Full detail to match gst-inspect-1.0 behavior
        let effective_detail = if use_json { detail } else { DetailLevel::Full };
        match registry::get_element(name, effective_detail) {
            Some(info) => {
                if use_json {
                    match serde_json::to_string_pretty(&info) {
                        Ok(json) => {
                            println!("{}", json);
                            0
                        }
                        Err(e) => {
                            error!("Failed to serialize element info: {}", e);
                            1
                        }
                    }
                } else {
                    print!("{}", inspect_format::format_element_text(&info));
                    0
                }
            }
            None => {
                error!("Element '{}' not found", name);
                1
            }
        }
    } else {
        // For text list output, force at least Summary to show long_name
        let list_detail = if use_json {
            detail
        } else {
            detail.max(DetailLevel::Summary)
        };
        let elements = registry::get_elements(list_detail);
        if use_json {
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
        } else {
            print!("{}", inspect_format::format_element_list_text(&elements));
            0
        }
    }
}
