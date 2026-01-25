//! Build script to generate the icons module from Lucide SVG files.
//!
//! This reads all SVG files from node_modules/lucide-static/icons/ and generates
//! a Rust file with a phf::Map mapping icon names to SVG strings.

use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("icons.rs");
    let mut file = BufWriter::new(File::create(&dest_path).unwrap());

    let icons_dir = Path::new("node_modules/lucide-static/icons");

    // Collect all SVG files
    let mut icons: Vec<(String, String)> = Vec::new();

    if icons_dir.exists() {
        for entry in fs::read_dir(icons_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "svg") {
                let file_name = path.file_stem().unwrap().to_str().unwrap();
                let svg_content = fs::read_to_string(&path).unwrap();

                // Process SVG: remove license comment and normalize
                let svg = process_svg(&svg_content);
                icons.push((file_name.to_string(), svg));
            }
        }
    }

    // Sort for deterministic output
    icons.sort_by(|a, b| a.0.cmp(&b.0));

    // Generate the phf map
    writeln!(
        file,
        "/// Auto-generated map of Lucide icon names to SVG strings."
    )
    .unwrap();
    writeln!(file, "/// Generated from {} icons.", icons.len()).unwrap();

    let mut map = phf_codegen::Map::new();
    for (name, svg) in &icons {
        map.entry(name.as_str(), &format!("{:?}", svg));
    }

    writeln!(
        file,
        "pub static ICONS: phf::Map<&'static str, &'static str> = {};",
        map.build()
    )
    .unwrap();

    // Tell Cargo to rerun if icons change
    println!("cargo:rerun-if-changed=node_modules/lucide-static/icons");
}

/// Process an SVG string: remove license comment and clean up attributes.
fn process_svg(svg: &str) -> String {
    let mut result = svg.to_string();

    // Remove license comment
    if let Some(start) = result.find("<!--") {
        if let Some(end) = result.find("-->") {
            result = result[..start].to_string() + &result[end + 3..];
        }
    }

    // Remove the class attribute (we'll add our own classes via CSS)
    result = remove_attribute(&result, "class");

    // Trim whitespace
    result = result.trim().to_string();

    // Normalize whitespace (collapse multiple spaces/newlines)
    let mut normalized = String::new();
    let mut prev_was_space = false;
    for c in result.chars() {
        if c.is_whitespace() {
            if !prev_was_space {
                normalized.push(' ');
                prev_was_space = true;
            }
        } else {
            normalized.push(c);
            prev_was_space = false;
        }
    }

    normalized
}

/// Remove an attribute from the SVG tag.
fn remove_attribute(svg: &str, attr: &str) -> String {
    // Simple regex-free removal of class="..."
    let pattern = format!("{}=\"", attr);
    if let Some(start) = svg.find(&pattern) {
        let after_attr = start + pattern.len();
        if let Some(end) = svg[after_attr..].find('"') {
            let end_pos = after_attr + end + 1;
            // Remove the attribute and any trailing space
            let mut result = svg[..start].to_string() + &svg[end_pos..];
            // Clean up double spaces
            result = result.replace("  ", " ");
            return result;
        }
    }
    svg.to_string()
}
