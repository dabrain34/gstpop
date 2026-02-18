// registry.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use gstreamer::prelude::*;
use gstreamer::{self as gst};
use serde::Serialize;
use std::str::FromStr;

/// Detail level for element information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    /// Only name and plugin_name
    None,
    /// Adds long_name, klass, description, author, rank
    Summary,
    /// Adds pad_templates
    Full,
}

impl FromStr for DetailLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(DetailLevel::None),
            "summary" => Ok(DetailLevel::Summary),
            "full" => Ok(DetailLevel::Full),
            other => Err(format!(
                "Invalid detail level: '{}'. Expected 'none', 'summary', or 'full'",
                other
            )),
        }
    }
}

/// Information about a GStreamer element factory.
#[derive(Debug, Clone, Serialize)]
pub struct ElementInfo {
    pub name: String,
    pub plugin_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub klass: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad_templates: Option<Vec<PadTemplateInfo>>,
}

/// Information about a pad template.
#[derive(Debug, Clone, Serialize)]
pub struct PadTemplateInfo {
    pub name: String,
    pub direction: String,
    pub presence: String,
    pub caps: String,
}

/// Query the GStreamer registry for all element factories.
///
/// The `detail` parameter controls how much information is returned:
/// - `None`: only name and plugin_name
/// - `Summary`: adds long_name, klass, description, author, rank
/// - `Full`: adds pad_templates
pub fn get_elements(detail: DetailLevel) -> Vec<ElementInfo> {
    let registry = gst::Registry::get();
    let mut elements: Vec<ElementInfo> = registry
        .features(gst::ElementFactory::static_type())
        .into_iter()
        .filter_map(|feature| feature.downcast::<gst::ElementFactory>().ok())
        .map(|factory| build_element_info(&factory, detail))
        .collect();

    elements.sort_by(|a, b| a.name.cmp(&b.name));
    elements
}

fn build_element_info(factory: &gst::ElementFactory, detail: DetailLevel) -> ElementInfo {
    let name = factory.name().to_string();
    let plugin_name = factory
        .plugin()
        .map(|p| p.plugin_name().to_string())
        .unwrap_or_default();

    let (long_name, klass, description, author, rank) = if detail >= DetailLevel::Summary {
        (
            Some(
                factory
                    .metadata(gst::ELEMENT_METADATA_LONGNAME)
                    .unwrap_or_default()
                    .to_string(),
            ),
            Some(
                factory
                    .metadata(gst::ELEMENT_METADATA_KLASS)
                    .unwrap_or_default()
                    .to_string(),
            ),
            Some(
                factory
                    .metadata(gst::ELEMENT_METADATA_DESCRIPTION)
                    .unwrap_or_default()
                    .to_string(),
            ),
            Some(
                factory
                    .metadata(gst::ELEMENT_METADATA_AUTHOR)
                    .unwrap_or_default()
                    .to_string(),
            ),
            Some(i32::from(factory.rank())),
        )
    } else {
        (None, None, None, None, None)
    };

    let pad_templates = if detail >= DetailLevel::Full {
        Some(
            factory
                .static_pad_templates()
                .iter()
                .map(|spt| {
                    let pt = spt.get();
                    PadTemplateInfo {
                        name: pt.name_template().to_string(),
                        direction: match pt.direction() {
                            gst::PadDirection::Src => "src".to_string(),
                            gst::PadDirection::Sink => "sink".to_string(),
                            _ => "unknown".to_string(),
                        },
                        presence: match pt.presence() {
                            gst::PadPresence::Always => "always".to_string(),
                            gst::PadPresence::Sometimes => "sometimes".to_string(),
                            gst::PadPresence::Request => "request".to_string(),
                        },
                        caps: pt.caps().to_string(),
                    }
                })
                .collect(),
        )
    } else {
        None
    };

    ElementInfo {
        name,
        plugin_name,
        long_name,
        klass,
        description,
        author,
        rank,
        pad_templates,
    }
}

impl PartialOrd for DetailLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DetailLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        fn to_num(d: &DetailLevel) -> u8 {
            match d {
                DetailLevel::None => 0,
                DetailLevel::Summary => 1,
                DetailLevel::Full => 2,
            }
        }
        to_num(self).cmp(&to_num(other))
    }
}
