// registry_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::gst::registry::{get_elements, DetailLevel};

#[test]
fn test_get_elements_none() {
    let _ = gstreamer::init();
    let elements = get_elements(DetailLevel::None);

    assert!(!elements.is_empty());
    assert!(elements.iter().any(|e| e.name == "fakesink"));

    // None level should only have name and plugin_name
    let fakesink = elements.iter().find(|e| e.name == "fakesink").unwrap();
    assert!(!fakesink.plugin_name.is_empty());
    assert!(fakesink.long_name.is_none());
    assert!(fakesink.klass.is_none());
    assert!(fakesink.description.is_none());
    assert!(fakesink.author.is_none());
    assert!(fakesink.rank.is_none());
    assert!(fakesink.pad_templates.is_none());
}

#[test]
fn test_get_elements_summary() {
    let _ = gstreamer::init();
    let elements = get_elements(DetailLevel::Summary);

    let fakesink = elements.iter().find(|e| e.name == "fakesink").unwrap();
    assert!(fakesink.long_name.is_some());
    assert!(fakesink.klass.is_some());
    assert!(fakesink.description.is_some());
    assert!(fakesink.author.is_some());
    assert!(fakesink.rank.is_some());
    // Summary should not include pad templates
    assert!(fakesink.pad_templates.is_none());
}

#[test]
fn test_get_elements_full() {
    let _ = gstreamer::init();
    let elements = get_elements(DetailLevel::Full);

    let fakesink = elements.iter().find(|e| e.name == "fakesink").unwrap();
    assert!(fakesink.long_name.is_some());
    assert!(fakesink.pad_templates.is_some());

    let pads = fakesink.pad_templates.as_ref().unwrap();
    assert!(!pads.is_empty());
    assert!(pads
        .iter()
        .any(|p| p.direction == "sink" && p.presence == "always"));
}

#[test]
fn test_elements_sorted() {
    let _ = gstreamer::init();
    let elements = get_elements(DetailLevel::None);

    for window in elements.windows(2) {
        assert!(
            window[0].name <= window[1].name,
            "Elements not sorted: '{}' > '{}'",
            window[0].name,
            window[1].name
        );
    }
}

#[test]
fn test_invalid_detail_level() {
    let result = "invalid".parse::<DetailLevel>();
    assert!(result.is_err());
}

#[test]
fn test_detail_level_parse() {
    assert_eq!("none".parse::<DetailLevel>().unwrap(), DetailLevel::None);
    assert_eq!(
        "summary".parse::<DetailLevel>().unwrap(),
        DetailLevel::Summary
    );
    assert_eq!("full".parse::<DetailLevel>().unwrap(), DetailLevel::Full);
}
