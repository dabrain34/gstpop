// inspect_format.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::fmt::Write;
use std::io::IsTerminal;

use super::registry::ElementInfo;

// ANSI color codes (used only when stdout is a terminal)
struct Colors {
    bold: &'static str,
    green: &'static str,
    cyan: &'static str,
    yellow: &'static str,
    blue: &'static str,
    reset: &'static str,
}

const COLORS_ON: Colors = Colors {
    bold: "\x1b[1m",
    green: "\x1b[32m",
    cyan: "\x1b[36m",
    yellow: "\x1b[33m",
    blue: "\x1b[34m",
    reset: "\x1b[0m",
};

const COLORS_OFF: Colors = Colors {
    bold: "",
    green: "",
    cyan: "",
    yellow: "",
    blue: "",
    reset: "",
};

fn colors() -> &'static Colors {
    if std::io::stdout().is_terminal() {
        &COLORS_ON
    } else {
        &COLORS_OFF
    }
}

/// Format a single element for terminal display (gst-inspect-1.0 style).
pub fn format_element_text(info: &ElementInfo) -> String {
    let mut out = String::new();

    format_factory_details(&mut out, info);
    format_plugin_details(&mut out, info);
    format_hierarchy(&mut out, info);
    format_pad_templates(&mut out, info);
    format_clocking(&mut out, info);
    format_uri_capabilities(&mut out, info);
    format_properties(&mut out, info);
    format_signals(&mut out, info);

    out
}

/// Format the element listing (all elements, gst-inspect-1.0 style).
pub fn format_element_list_text(elements: &[ElementInfo]) -> String {
    let mut out = String::new();
    let c = colors();
    for el in elements {
        let long = el.long_name.as_deref().unwrap_or("");
        if el.plugin_name.is_empty() {
            let _ = writeln!(out, "{}{}{}: {}", c.green, el.name, c.reset, long);
        } else {
            let _ = writeln!(
                out,
                "{}:  {}{}{}: {}",
                el.plugin_name, c.green, el.name, c.reset, long
            );
        }
    }
    out
}

fn format_factory_details(out: &mut String, info: &ElementInfo) {
    let c = colors();
    let _ = writeln!(out, "{}Factory Details:{}", c.bold, c.reset);
    detail_line(out, "Rank", &format_rank(info.rank.unwrap_or(0)));
    if let Some(ref v) = info.long_name {
        detail_line(out, "Long-name", v);
    }
    if let Some(ref v) = info.klass {
        detail_line(out, "Klass", v);
    }
    if let Some(ref v) = info.description {
        detail_line(out, "Description", v);
    }
    if let Some(ref v) = info.author {
        detail_line(out, "Author", v);
    }
    let _ = writeln!(out);
}

fn format_plugin_details(out: &mut String, info: &ElementInfo) {
    let pi = match &info.plugin_info {
        Some(pi) => pi,
        None => return,
    };
    let c = colors();
    let _ = writeln!(out, "{}Plugin Details:{}", c.bold, c.reset);
    detail_line(out, "Name", &pi.name);
    detail_line(out, "Description", &pi.description);
    detail_line(out, "Filename", pi.filename.as_deref().unwrap_or("(null)"));
    detail_line(out, "Version", &pi.version);
    detail_line(out, "License", &pi.license);
    detail_line(out, "Source module", &pi.source);
    if let Some(ref rd) = pi.release_date {
        detail_line(out, "Source release date", rd);
    }
    detail_line(out, "Binary package", &pi.package);
    detail_line(out, "Origin URL", &pi.origin);
    let _ = writeln!(out);
}

fn format_hierarchy(out: &mut String, info: &ElementInfo) {
    let hierarchy = match &info.hierarchy {
        Some(h) if !h.is_empty() => h,
        _ => return,
    };

    let c = colors();
    for (i, type_name) in hierarchy.iter().enumerate() {
        if i == 0 {
            let _ = writeln!(out, "{}{}{}", c.green, type_name, c.reset);
        } else {
            let indent = " ".repeat((i - 1) * 6 + 1);
            let _ = writeln!(out, "{}+----{}{}{}", indent, c.green, type_name, c.reset);
        }
    }
    let _ = writeln!(out);
}

fn format_pad_templates(out: &mut String, info: &ElementInfo) {
    let pads = match &info.pad_templates {
        Some(p) => p,
        None => return,
    };

    if pads.is_empty() {
        let _ = writeln!(out, "Element has no pad templates.");
        let _ = writeln!(out);
        return;
    }

    let c = colors();
    let _ = writeln!(out, "{}Pad Templates:{}", c.bold, c.reset);
    for pt in pads {
        let dir_label = match pt.direction.as_str() {
            "src" => "SRC",
            "sink" => "SINK",
            _ => "UNKNOWN",
        };
        let presence_label = match pt.presence.as_str() {
            "always" => "Always",
            "sometimes" => "Sometimes",
            "request" => "On request",
            _ => &pt.presence,
        };
        let _ = writeln!(
            out,
            "  {} template: {}'{}'{}",
            dir_label, c.yellow, pt.name, c.reset
        );
        let _ = writeln!(out, "    Availability: {}", presence_label);

        if pt.caps == "ANY" || pt.caps == "EMPTY" {
            let _ = writeln!(out, "    Capabilities:");
            let _ = writeln!(out, "      {}", pt.caps);
        } else {
            let _ = writeln!(out, "    Capabilities:");
            format_caps(out, &pt.caps);
        }
        let _ = writeln!(out);
    }
}

fn format_caps(out: &mut String, caps_str: &str) {
    let c = colors();
    // Caps string format: "media/type, field1=(type)val1, field2=(type)val2; media/type2, ..."
    // Split on "; " for multiple structures
    for structure in caps_str.split("; ") {
        let structure = structure.trim();
        if structure.is_empty() {
            continue;
        }
        // Split on first comma to get media type and fields
        if let Some(comma_pos) = structure.find(", ") {
            let media_type = &structure[..comma_pos];
            let _ = writeln!(out, "      {}{}{}", c.blue, media_type, c.reset);
            let fields = &structure[comma_pos + 2..];
            // Split fields - but be careful about nested braces/parens
            for field in split_caps_fields(fields) {
                let field = field.trim();
                if let Some(eq_pos) = field.find('=') {
                    let name = field[..eq_pos].trim();
                    let value = field[eq_pos + 1..].trim();
                    let _ = writeln!(out, "        {:>15}: {}", name, value);
                }
            }
        } else {
            let _ = writeln!(out, "      {}", structure);
        }
    }
}

fn split_caps_fields(fields: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in fields.char_indices() {
        match c {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth = (depth - 1).max(0),
            ',' if depth == 0 => {
                result.push(&fields[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < fields.len() {
        result.push(&fields[start..]);
    }
    result
}

fn format_clocking(out: &mut String, info: &ElementInfo) {
    match info.has_clocking {
        Some(true) => {
            let _ = writeln!(out, "Element provides a clock.");
        }
        _ => {
            let _ = writeln!(out, "Element has no clocking capabilities.");
        }
    }
    let _ = writeln!(out);
}

fn format_uri_capabilities(out: &mut String, info: &ElementInfo) {
    match &info.uri_info {
        Some(uri) => {
            let _ = writeln!(out, "URI handling capabilities:");
            let _ = writeln!(out, "  Element can act as {}.", uri.uri_type);
            let _ = writeln!(out, "  Supported URI protocols:");
            for proto in &uri.protocols {
                let _ = writeln!(out, "    {}", proto);
            }
        }
        None => {
            let _ = writeln!(out, "Element has no URI handling capabilities.");
        }
    }
    let _ = writeln!(out);
}

fn format_properties(out: &mut String, info: &ElementInfo) {
    let props = match &info.properties {
        Some(p) if !p.is_empty() => p,
        _ => return,
    };

    let c = colors();
    let _ = writeln!(out, "{}Element Properties:{}", c.bold, c.reset);
    for prop in props {
        // Property name and blurb
        let blurb = prop.blurb.as_deref().unwrap_or("No description");
        let _ = writeln!(out, "  {}{:<20}{}: {}", c.cyan, prop.name, c.reset, blurb);

        // Flags
        let _ = writeln!(out, "                        flags: {}", prop.flags);

        // Type, range, default
        if let Some(ref range) = prop.range {
            let _ = writeln!(
                out,
                "                        {}. Range: {} Default: {} ",
                prop.value_type, range, prop.default_value
            );
        } else {
            let _ = writeln!(
                out,
                "                        {}. Default: {}",
                prop.value_type, prop.default_value
            );
        }

        // Enum/Flag values
        if let Some(ref values) = prop.enum_values {
            for v in values {
                let _ = writeln!(
                    out,
                    "                           ({:>3}): {:<16} - {}",
                    v.value, v.nick, v.name
                );
            }
        }

        let _ = writeln!(out);
    }
}

fn format_signals(out: &mut String, info: &ElementInfo) {
    let signals = match &info.signals {
        Some(s) if !s.is_empty() => s,
        _ => return,
    };

    let c = colors();
    let _ = writeln!(out, "{}Element Signals:{}", c.bold, c.reset);
    for sig in signals {
        let params = if sig.param_types.is_empty() {
            String::new()
        } else {
            sig.param_types.join(", ")
        };
        let _ = writeln!(
            out,
            "  {}\"{}\"{}  : {} ({}) ",
            c.yellow, sig.name, c.reset, sig.return_type, params
        );
    }
    let _ = writeln!(out);
}

fn detail_line(out: &mut String, label: &str, value: &str) {
    let _ = writeln!(out, "  {:<25}{}", label, value);
}

fn format_rank(rank: i32) -> String {
    let name = match rank {
        0 => "none",
        64 => "marginal",
        128 => "secondary",
        256 => "primary",
        _ => "",
    };
    if name.is_empty() {
        format!("{}", rank)
    } else {
        format!("{} ({})", name, rank)
    }
}
