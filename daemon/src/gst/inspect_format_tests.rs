// inspect_format_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::gst::inspect_format;
use crate::gst::registry::{
    ElementInfo, EnumValueInfo, PadTemplateInfo, PluginInfo, PropertyInfo, SignalInfo, UriInfo,
};

fn minimal_element() -> ElementInfo {
    ElementInfo {
        name: "testsrc".to_string(),
        plugin_name: "testplugin".to_string(),
        long_name: None,
        klass: None,
        description: None,
        author: None,
        rank: None,
        pad_templates: None,
        plugin_info: None,
        hierarchy: None,
        properties: None,
        signals: None,
        uri_info: None,
        has_clocking: None,
    }
}

#[test]
fn test_format_element_no_pads_no_props_no_signals() {
    let info = minimal_element();
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Factory Details:"));
    // Should not panic or contain "Pad Templates:" when pads are None
    assert!(!output.contains("Pad Templates:"));
    assert!(!output.contains("Element Properties:"));
    assert!(!output.contains("Element Signals:"));
}

#[test]
fn test_format_element_empty_pads() {
    let mut info = minimal_element();
    info.pad_templates = Some(vec![]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Element has no pad templates."));
}

#[test]
fn test_format_element_with_pad_templates() {
    let mut info = minimal_element();
    info.pad_templates = Some(vec![PadTemplateInfo {
        name: "src".to_string(),
        direction: "src".to_string(),
        presence: "always".to_string(),
        caps: "video/x-raw, format=(string)NV12, width=(int)1920".to_string(),
    }]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Pad Templates:"));
    assert!(output.contains("SRC template:"));
    assert!(output.contains("Availability: Always"));
    assert!(output.contains("video/x-raw"));
}

#[test]
fn test_format_element_caps_any() {
    let mut info = minimal_element();
    info.pad_templates = Some(vec![PadTemplateInfo {
        name: "sink".to_string(),
        direction: "sink".to_string(),
        presence: "always".to_string(),
        caps: "ANY".to_string(),
    }]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("ANY"));
}

#[test]
fn test_format_element_caps_empty() {
    let mut info = minimal_element();
    info.pad_templates = Some(vec![PadTemplateInfo {
        name: "sink".to_string(),
        direction: "sink".to_string(),
        presence: "always".to_string(),
        caps: "EMPTY".to_string(),
    }]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("EMPTY"));
}

#[test]
fn test_format_element_with_uri_info() {
    let mut info = minimal_element();
    info.uri_info = Some(UriInfo {
        uri_type: "source".to_string(),
        protocols: vec!["file".to_string(), "http".to_string()],
    });
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("URI handling capabilities:"));
    assert!(output.contains("Element can act as source."));
    assert!(output.contains("file"));
    assert!(output.contains("http"));
}

#[test]
fn test_format_element_no_uri() {
    let info = minimal_element();
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Element has no URI handling capabilities."));
}

#[test]
fn test_format_element_with_clocking() {
    let mut info = minimal_element();
    info.has_clocking = Some(true);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Element provides a clock."));
}

#[test]
fn test_format_element_no_clocking() {
    let mut info = minimal_element();
    info.has_clocking = Some(false);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Element has no clocking capabilities."));
}

#[test]
fn test_format_element_with_properties() {
    let mut info = minimal_element();
    info.properties = Some(vec![PropertyInfo {
        name: "bitrate".to_string(),
        blurb: Some("Encoding bitrate".to_string()),
        flags: "readable, writable".to_string(),
        value_type: "Integer".to_string(),
        default_value: "128000".to_string(),
        range: Some("0 - 2147483647".to_string()),
        enum_values: None,
    }]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Element Properties:"));
    assert!(output.contains("bitrate"));
    assert!(output.contains("Encoding bitrate"));
    assert!(output.contains("readable, writable"));
}

#[test]
fn test_format_element_with_enum_property() {
    let mut info = minimal_element();
    info.properties = Some(vec![PropertyInfo {
        name: "mode".to_string(),
        blurb: Some("Operating mode".to_string()),
        flags: "readable, writable".to_string(),
        value_type: "Enum \"GstTestMode\"".to_string(),
        default_value: "0, \"auto\"".to_string(),
        range: None,
        enum_values: Some(vec![
            EnumValueInfo {
                value: 0,
                nick: "auto".to_string(),
                name: "Automatic".to_string(),
            },
            EnumValueInfo {
                value: 1,
                nick: "manual".to_string(),
                name: "Manual".to_string(),
            },
        ]),
    }]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("auto"));
    assert!(output.contains("Automatic"));
    assert!(output.contains("manual"));
    assert!(output.contains("Manual"));
}

#[test]
fn test_format_element_with_signals() {
    let mut info = minimal_element();
    info.signals = Some(vec![SignalInfo {
        name: "handoff".to_string(),
        return_type: "void".to_string(),
        param_types: vec!["GstBuffer".to_string(), "GstPad".to_string()],
    }]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Element Signals:"));
    assert!(output.contains("handoff"));
    assert!(output.contains("GstBuffer, GstPad"));
}

#[test]
fn test_format_element_with_hierarchy() {
    let mut info = minimal_element();
    info.hierarchy = Some(vec![
        "GObject".to_string(),
        "GInitiallyUnowned".to_string(),
        "GstObject".to_string(),
        "GstElement".to_string(),
    ]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("GObject"));
    assert!(output.contains("GstElement"));
    assert!(output.contains("+----"));
}

#[test]
fn test_format_element_with_plugin_info() {
    let mut info = minimal_element();
    info.plugin_info = Some(PluginInfo {
        name: "coreelements".to_string(),
        description: "GStreamer core elements".to_string(),
        filename: Some("/usr/lib/gstreamer-1.0/libgstcoreelements.so".to_string()),
        version: "1.24.0".to_string(),
        license: "LGPL".to_string(),
        source: "gstreamer".to_string(),
        release_date: Some("2024-03-04".to_string()),
        package: "GStreamer".to_string(),
        origin: "https://gstreamer.freedesktop.org".to_string(),
    });
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("Plugin Details:"));
    assert!(output.contains("coreelements"));
    assert!(output.contains("1.24.0"));
    assert!(output.contains("LGPL"));
}

#[test]
fn test_format_element_list_text() {
    let elements = vec![
        ElementInfo {
            name: "fakesink".to_string(),
            plugin_name: "coreelements".to_string(),
            long_name: Some("Fake Sink".to_string()),
            ..minimal_element()
        },
        ElementInfo {
            name: "fakesrc".to_string(),
            plugin_name: "coreelements".to_string(),
            long_name: Some("Fake Source".to_string()),
            ..minimal_element()
        },
    ];
    let output = inspect_format::format_element_list_text(&elements);
    assert!(output.contains("fakesink"));
    assert!(output.contains("Fake Sink"));
    assert!(output.contains("fakesrc"));
    assert!(output.contains("coreelements"));
}

#[test]
fn test_format_element_list_no_plugin() {
    let elements = vec![ElementInfo {
        name: "orphan".to_string(),
        plugin_name: String::new(),
        long_name: Some("Orphan Element".to_string()),
        ..minimal_element()
    }];
    let output = inspect_format::format_element_list_text(&elements);
    assert!(output.contains("orphan"));
    assert!(output.contains("Orphan Element"));
    // Should not contain ":" prefix from plugin name
    assert!(!output.contains(":  "));
}

#[test]
fn test_format_rank_known_values() {
    let mut info = minimal_element();
    info.rank = Some(0);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("none (0)"));

    info.rank = Some(256);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("primary (256)"));
}

#[test]
fn test_format_rank_unknown_value() {
    let mut info = minimal_element();
    info.rank = Some(999);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("999"));
}

#[test]
fn test_format_caps_nested_brackets() {
    let mut info = minimal_element();
    info.pad_templates = Some(vec![PadTemplateInfo {
        name: "src".to_string(),
        direction: "src".to_string(),
        presence: "always".to_string(),
        caps: "audio/x-raw, rate=(int)[ 1, 2147483647 ], channels=(int)[ 1, 64 ]".to_string(),
    }]);
    let output = inspect_format::format_element_text(&info);
    assert!(output.contains("audio/x-raw"));
    assert!(output.contains("rate"));
    assert!(output.contains("channels"));
}
