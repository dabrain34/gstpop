// registry.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use gstreamer::prelude::*;
use gstreamer::{self as gst, glib};
use serde::Serialize;
use std::str::FromStr;
use tracing::warn;

/// Detail level for element information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    /// Only name and plugin_name
    None,
    /// Adds long_name, klass, description, author, rank
    Summary,
    /// Adds pad_templates, plugin_info, hierarchy, properties, signals, uri_info
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_info: Option<PluginInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hierarchy: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<PropertyInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signals: Option<Vec<SignalInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri_info: Option<UriInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_clocking: Option<bool>,
}

/// Information about a pad template.
#[derive(Debug, Clone, Serialize)]
pub struct PadTemplateInfo {
    pub name: String,
    pub direction: String,
    pub presence: String,
    pub caps: String,
}

/// Information about a GStreamer plugin.
#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
    pub filename: Option<String>,
    pub version: String,
    pub license: String,
    pub source: String,
    pub release_date: Option<String>,
    pub package: String,
    pub origin: String,
}

/// Information about an element property.
#[derive(Debug, Clone, Serialize)]
pub struct PropertyInfo {
    pub name: String,
    pub blurb: Option<String>,
    pub flags: String,
    pub value_type: String,
    pub default_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<EnumValueInfo>>,
}

/// Information about an enum or flags value.
#[derive(Debug, Clone, Serialize)]
pub struct EnumValueInfo {
    pub value: i64,
    pub nick: String,
    pub name: String,
}

/// Information about an element signal.
#[derive(Debug, Clone, Serialize)]
pub struct SignalInfo {
    pub name: String,
    pub return_type: String,
    pub param_types: Vec<String>,
}

/// Information about URI handling capabilities.
#[derive(Debug, Clone, Serialize)]
pub struct UriInfo {
    pub uri_type: String,
    pub protocols: Vec<String>,
}

/// Query the GStreamer registry for all element factories.
///
/// The `detail` parameter controls how much information is returned:
/// - `None`: only name and plugin_name
/// - `Summary`: adds long_name, klass, description, author, rank
/// - `Full`: adds pad_templates, plugin_info, hierarchy, properties, signals, uri_info
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

/// Query the GStreamer registry for a single element factory by name.
///
/// Returns `None` if the element is not found.
pub fn get_element(name: &str, detail: DetailLevel) -> Option<ElementInfo> {
    let registry = gst::Registry::get();
    registry
        .find_feature(name, gst::ElementFactory::static_type())
        .and_then(|feature| feature.downcast::<gst::ElementFactory>().ok())
        .map(|factory| build_element_info(&factory, detail))
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

    let (pad_templates, plugin_info, hierarchy, properties, signals, uri_info, has_clocking) =
        if detail >= DetailLevel::Full {
            // Create a temporary element instance for property/hierarchy introspection
            let element = factory.create().build().ok();
            if element.is_none() {
                warn!(
                    "Could not create element '{}' for introspection",
                    factory.name()
                );
            }
            let hierarchy = element.as_ref().map(build_hierarchy);
            let properties = element.as_ref().map(build_properties);
            let signals = build_signals(factory, element.as_ref());
            let has_clocking = element.as_ref().map(|e| e.provide_clock().is_some());
            (
                Some(build_pad_templates(factory)),
                build_plugin_info(factory),
                hierarchy,
                properties,
                signals,
                build_uri_info(factory),
                has_clocking,
            )
        } else {
            (None, None, None, None, None, None, None)
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
        plugin_info,
        hierarchy,
        properties,
        signals,
        uri_info,
        has_clocking,
    }
}

fn build_pad_templates(factory: &gst::ElementFactory) -> Vec<PadTemplateInfo> {
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
        .collect()
}

fn build_plugin_info(factory: &gst::ElementFactory) -> Option<PluginInfo> {
    factory.plugin().map(|plugin| PluginInfo {
        name: plugin.plugin_name().to_string(),
        description: plugin.description().to_string(),
        filename: plugin.filename().map(|p| p.to_string_lossy().to_string()),
        version: plugin.version().to_string(),
        license: plugin.license().to_string(),
        source: plugin.source().to_string(),
        release_date: plugin.release_date_string().map(|s| s.to_string()),
        package: plugin.package().to_string(),
        origin: plugin.origin().to_string(),
    })
}

fn build_hierarchy(element: &gst::Element) -> Vec<String> {
    let mut hierarchy = Vec::new();
    let mut current_type = element.type_();
    while current_type.is_valid() && current_type != glib::Type::INVALID {
        hierarchy.push(current_type.name().to_string());
        match current_type.parent() {
            Some(parent) if parent != current_type => current_type = parent,
            _ => break,
        }
    }
    hierarchy.reverse();
    hierarchy
}

fn build_properties(element: &gst::Element) -> Vec<PropertyInfo> {
    element
        .list_properties()
        .iter()
        .map(build_property_info)
        .collect()
}

fn build_property_info(pspec: &glib::ParamSpec) -> PropertyInfo {
    let name = pspec.name().to_string();
    let blurb = pspec.blurb().map(|s| s.to_string());

    let flags = build_param_flags(pspec);
    let value_type_name = pspec.value_type().name().to_string();
    let (value_type, default_value, range, enum_values) =
        build_property_type_info(pspec, &value_type_name);

    PropertyInfo {
        name,
        blurb,
        flags,
        value_type,
        default_value,
        range,
        enum_values,
    }
}

fn build_param_flags(pspec: &glib::ParamSpec) -> String {
    let f = pspec.flags();
    let mut parts = Vec::new();
    if f.contains(glib::ParamFlags::READABLE) {
        parts.push("readable");
    }
    if f.contains(glib::ParamFlags::WRITABLE) {
        parts.push("writable");
    }
    if f.contains(glib::ParamFlags::CONSTRUCT) {
        parts.push("construct");
    }
    if f.contains(glib::ParamFlags::CONSTRUCT_ONLY) {
        parts.push("construct only");
    }
    if f.contains(glib::ParamFlags::LAX_VALIDATION) {
        parts.push("lax validation");
    }
    if f.contains(glib::ParamFlags::DEPRECATED) {
        parts.push("deprecated");
    }
    // GStreamer-specific flags: G_PARAM_USER_SHIFT = 8
    let bits = f.bits();
    // GST_PARAM_CONTROLLABLE = 1 << (8+1) = 512
    if bits & (1 << 9) != 0 {
        parts.push("controllable");
    }
    // GST_PARAM_MUTABLE_READY = 1 << (8+2) = 1024
    if bits & (1 << 10) != 0 {
        parts.push("changeable only in NULL or READY state");
    }
    // GST_PARAM_MUTABLE_PAUSED = 1 << (8+3) = 2048
    if bits & (1 << 11) != 0 {
        parts.push("changeable only in NULL, READY or PAUSED state");
    }
    // GST_PARAM_MUTABLE_PLAYING = 1 << (8+4) = 4096
    if bits & (1 << 12) != 0 {
        parts.push("changeable in NULL, READY, PAUSED or PLAYING state");
    }
    // GST_PARAM_CONDITIONALLY_AVAILABLE = 1 << (8+6) = 16384
    if bits & (1 << 14) != 0 {
        parts.push("conditionally available");
    }
    parts.join(", ")
}

fn build_property_type_info(
    pspec: &glib::ParamSpec,
    type_name: &str,
) -> (String, String, Option<String>, Option<Vec<EnumValueInfo>>) {
    if let Some(p) = pspec.downcast_ref::<glib::ParamSpecBoolean>() {
        (
            "Boolean".to_string(),
            format!("{}", p.default_value()),
            None,
            None,
        )
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecInt>() {
        (
            "Integer".to_string(),
            format!("{}", p.default_value()),
            Some(format!("{} - {}", p.minimum(), p.maximum())),
            None,
        )
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecUInt>() {
        (
            "Unsigned Integer".to_string(),
            format!("{}", p.default_value()),
            Some(format!("{} - {}", p.minimum(), p.maximum())),
            None,
        )
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecInt64>() {
        (
            "Integer64".to_string(),
            format!("{}", p.default_value()),
            Some(format!("{} - {}", p.minimum(), p.maximum())),
            None,
        )
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecUInt64>() {
        (
            "Unsigned Integer64".to_string(),
            format!("{}", p.default_value()),
            Some(format!("{} - {}", p.minimum(), p.maximum())),
            None,
        )
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecFloat>() {
        (
            "Float".to_string(),
            format!("{}", p.default_value()),
            Some(format!("{} - {}", p.minimum(), p.maximum())),
            None,
        )
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecDouble>() {
        (
            "Double".to_string(),
            format!("{}", p.default_value()),
            Some(format!("{} - {}", p.minimum(), p.maximum())),
            None,
        )
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecString>() {
        let default = p
            .default_value()
            .map(|s| format!("\"{}\"", s))
            .unwrap_or_else(|| "null".to_string());
        ("String".to_string(), default, None, None)
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecEnum>() {
        let enum_class = p.enum_class();
        let default_val = p.default_value_as_i32();
        let default_nick = enum_class
            .value(default_val)
            .map(|v| v.nick().to_string())
            .unwrap_or_default();
        let values: Vec<EnumValueInfo> = enum_class
            .values()
            .iter()
            .map(|v| EnumValueInfo {
                value: v.value() as i64,
                nick: v.nick().to_string(),
                name: v.name().to_string(),
            })
            .collect();
        (
            format!("Enum \"{}\"", type_name),
            format!("{}, \"{}\"", default_val, default_nick),
            None,
            Some(values),
        )
    } else if let Some(p) = pspec.downcast_ref::<glib::ParamSpecFlags>() {
        let flags_class = p.flags_class();
        let default_val = p.default_value_as_u32();
        let values: Vec<EnumValueInfo> = flags_class
            .values()
            .iter()
            .map(|v| EnumValueInfo {
                value: v.value() as i64,
                nick: v.nick().to_string(),
                name: v.name().to_string(),
            })
            .collect();
        (
            format!("Flags \"{}\"", type_name),
            format!("0x{:08x}", default_val),
            None,
            Some(values),
        )
    } else if pspec.downcast_ref::<glib::ParamSpecObject>().is_some() {
        (
            format!("Object of type \"{}\"", type_name),
            "null".to_string(),
            None,
            None,
        )
    } else if pspec.downcast_ref::<glib::ParamSpecBoxed>().is_some() {
        (
            format!("Boxed of type \"{}\"", type_name),
            String::new(),
            None,
            None,
        )
    } else {
        (type_name.to_string(), String::new(), None, None)
    }
}

fn build_signals(
    factory: &gst::ElementFactory,
    element: Option<&gst::Element>,
) -> Option<Vec<SignalInfo>> {
    let element_type = element
        .map(|e| e.type_())
        .unwrap_or_else(|| factory.element_type());
    if !element_type.is_valid() {
        return None;
    }

    let signals = collect_signals_for_type(element_type);
    if signals.is_empty() {
        None
    } else {
        Some(signals)
    }
}

fn collect_signals_for_type(type_: glib::Type) -> Vec<SignalInfo> {
    use glib::translate::*;
    use std::collections::HashSet;

    let mut all_signals = Vec::new();
    let mut seen_ids = HashSet::new();

    // Walk the type hierarchy to collect signals from all ancestor types
    let mut current = type_;
    while current.is_valid() && current != glib::Type::INVALID {
        let mut n_ids: std::ffi::c_uint = 0;
        let ids_ptr =
            unsafe { glib::gobject_ffi::g_signal_list_ids(current.into_glib(), &mut n_ids) };

        if !ids_ptr.is_null() && n_ids > 0 {
            let ids: Vec<std::ffi::c_uint> =
                unsafe { std::slice::from_raw_parts(ids_ptr, n_ids as usize) }.to_vec();
            unsafe {
                glib::ffi::g_free(ids_ptr as *mut _);
            }

            for &id in &ids {
                if !seen_ids.insert(id) {
                    continue;
                }
                let mut query = std::mem::MaybeUninit::<glib::gobject_ffi::GSignalQuery>::uninit();
                unsafe {
                    glib::gobject_ffi::g_signal_query(id, query.as_mut_ptr());
                }
                let query = unsafe { query.assume_init() };
                if query.signal_id == 0 {
                    continue;
                }
                let signal_name = unsafe { std::ffi::CStr::from_ptr(query.signal_name) }
                    .to_string_lossy()
                    .to_string();
                let return_type_gtype: glib::Type = unsafe { from_glib(query.return_type) };
                let return_type = return_type_gtype.name().to_string();
                let param_types: Vec<String> = if query.n_params > 0 && !query.param_types.is_null()
                {
                    let params = unsafe {
                        std::slice::from_raw_parts(query.param_types, query.n_params as usize)
                    };
                    params
                        .iter()
                        .map(|&gt| {
                            let t: glib::Type = unsafe { from_glib(gt) };
                            t.name().to_string()
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                all_signals.push(SignalInfo {
                    name: signal_name,
                    return_type,
                    param_types,
                });
            }
        } else if !ids_ptr.is_null() {
            unsafe {
                glib::ffi::g_free(ids_ptr as *mut _);
            }
        }

        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }

    all_signals
}

fn build_uri_info(factory: &gst::ElementFactory) -> Option<UriInfo> {
    let uri_type = factory.uri_type();
    if uri_type == gst::URIType::Unknown {
        return None;
    }
    let protocols: Vec<String> = factory
        .uri_protocols()
        .iter()
        .map(|s| s.to_string())
        .collect();
    Some(UriInfo {
        uri_type: match uri_type {
            gst::URIType::Sink => "sink".to_string(),
            gst::URIType::Src => "source".to_string(),
            _ => "unknown".to_string(),
        },
        protocols,
    })
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
