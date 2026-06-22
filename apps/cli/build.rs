use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=TREEASE_WEB_ASSET_BASE_URL");

    let manifest_path =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set"))
            .join("Cargo.toml");
    let version = read_wasm_release_date(&manifest_path)
        .expect("Cargo.toml must define package.metadata.treease.wasm_release_date");
    let base_url = env::var("TREEASE_WEB_ASSET_BASE_URL")
        .unwrap_or_else(|_| "https://treease.com/cli-assets".to_string());

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR should be set"));
    let generated_path = out_dir.join("treease_web_config.rs");
    fs::write(
        generated_path,
        format!(
            "pub(super) const WEB_ASSET_VERSION: &str = {:?};\n\
             pub(super) const DEFAULT_WEB_ASSET_BASE_URL: &str = {:?};\n",
            version, base_url
        ),
    )
    .expect("generated web config should be writable");
}

fn read_wasm_release_date(manifest_path: &Path) -> Option<String> {
    let manifest = fs::read_to_string(manifest_path).ok()?;
    manifest.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("wasm_release_date") {
            return None;
        }
        let (_, value) = trimmed.split_once('=')?;
        let value = value.trim().trim_matches('"');
        if value.len() == 8 && value.chars().all(|ch| ch.is_ascii_digit()) {
            Some(value.to_string())
        } else {
            None
        }
    })
}
