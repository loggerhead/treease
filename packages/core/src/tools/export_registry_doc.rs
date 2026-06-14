use std::fs;
use std::path::{Path, PathBuf};

const OPS_TEXT: &str = include_str!("../operators/registry_tables_ops.rs");
const FORMATS_TEXT: &str = include_str!("../operators/registry_tables_formats.rs");

const BUILD_OPTION_LINES: &[&str] = &[
    "- lang_json: true",
    "- lang_yaml: true",
    "- lang_toml: true",
    "- lang_python: true",
    "- lang_javascript: true",
    "- lang_csv: true",
    "- op_traversal: true",
    "- op_math: true",
    "- op_relational: true",
    "- op_logic: true",
    "- op_assign: true",
    "- op_collection: true",
    "- op_codec: true",
    "- op_strings: true",
    "- op_sort: true",
    "- op_meta: true",
    "- op_special: true",
];

fn collect_op_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut start = 0;
    let needle = "id: ";

    while let Some(offset) = text[start..].find(needle) {
        let name_start = start + offset + needle.len();
        let Some(name_end_rel) = text[name_start..].find(".id") else {
            break;
        };
        let name_end = name_start + name_end_rel;
        let name = text[name_start..name_end].trim();
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
        {
            start = name_end;
            continue;
        }
        let lowered = name.to_ascii_lowercase();
        if seen.insert(lowered.clone()) {
            out.push(lowered);
        }
        start = name_end;
    }

    out
}

fn collect_format_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    let needle = "name: \"";

    while let Some(offset) = text[start..].find(needle) {
        let name_start = start + offset + needle.len();
        let Some(name_end_rel) = text[name_start..].find('"') else {
            break;
        };
        let name_end = name_start + name_end_rel;
        out.push(text[name_start..name_end].to_string());
        start = name_end;
    }

    out
}

fn build_output() -> String {
    let mut out = String::new();
    out.push_str("# Core Registry Capabilities\n\n");
    out.push_str("## Build Options\n");
    for line in BUILD_OPTION_LINES {
        out.push_str(line);
        out.push('\n');
    }

    out.push_str("\n## Operators\n");
    for name in collect_op_names(OPS_TEXT) {
        out.push_str("- ");
        out.push_str(&name);
        out.push('\n');
    }

    out.push_str("\n## Formats\n");
    for name in collect_format_names(FORMATS_TEXT) {
        out.push_str("- ");
        out.push_str(&name);
        out.push('\n');
    }

    out
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages directory")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn write_output(contents: &str) -> std::io::Result<()> {
    let out_dir = repo_root().join("docs/generated");
    fs::create_dir_all(&out_dir)?;
    fs::write(out_dir.join("core-registry-capabilities.md"), contents)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let contents = build_output();
    write_output(&contents)?;
    Ok(())
}
