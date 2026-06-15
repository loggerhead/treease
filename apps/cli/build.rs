use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=TREEASE_WEB_DIST");
    println!("cargo:rerun-if-env-changed=TREEASE_REQUIRE_WEB_ASSETS");

    let dist = env::var_os("TREEASE_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../apps/web/build"));
    println!("cargo:rerun-if-changed={}", dist.display());

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR should be set"));
    let generated_path = out_dir.join("treease_web_assets.rs");

    if !dist.is_dir() {
        if env::var("TREEASE_REQUIRE_WEB_ASSETS").as_deref() == Ok("1") {
            panic!(
                "web assets are required but missing at {}. Run `cd apps/web && pnpm build` or set TREEASE_WEB_DIST.",
                dist.display()
            );
        }
        fs::write(&generated_path, unavailable_manifest())
            .expect("generated web asset manifest should be writable");
        return;
    }

    let dist = fs::canonicalize(&dist).unwrap_or_else(|error| {
        panic!(
            "failed to resolve web asset directory {}: {error}",
            dist.display()
        )
    });

    let mut assets = Vec::new();
    collect_assets(&dist, &dist, &mut assets);
    assets.sort_by(|left, right| left.route_path.cmp(&right.route_path));

    fs::write(&generated_path, available_manifest(&assets))
        .expect("generated web asset manifest should be writable");
}

#[derive(Debug)]
struct AssetEntry {
    route_path: String,
    content_type: &'static str,
    source_path: PathBuf,
}

fn collect_assets(root: &Path, dir: &Path, assets: &mut Vec<AssetEntry>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| {
        panic!(
            "failed to read web asset directory {}: {error}",
            dir.display()
        )
    });

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read web asset entry under {}: {error}",
                dir.display()
            )
        });
        let path = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!("failed to inspect web asset {}: {error}", path.display())
        });

        if file_type.is_dir() {
            collect_assets(root, &path, assets);
        } else if file_type.is_file() {
            let relative_path = path
                .strip_prefix(root)
                .expect("asset path should be under web dist root");
            let route_path = format!("/{}", relative_path.to_string_lossy().replace('\\', "/"));
            assets.push(AssetEntry {
                content_type: content_type_for(&path),
                route_path,
                source_path: path,
            });
        }
    }
}

fn unavailable_manifest() -> String {
    [
        "pub(super) const WEB_ASSETS_AVAILABLE: bool = false;\n",
        "pub(super) static WEB_ASSETS: &[EmbeddedAsset] = &[];\n",
    ]
    .concat()
}

fn available_manifest(assets: &[AssetEntry]) -> String {
    let mut generated = String::from("pub(super) const WEB_ASSETS_AVAILABLE: bool = true;\n");
    generated.push_str("pub(super) static WEB_ASSETS: &[EmbeddedAsset] = &[\n");
    for asset in assets {
        generated.push_str("    EmbeddedAsset {\n");
        generated.push_str(&format!("        path: {:?},\n", asset.route_path));
        generated.push_str(&format!(
            "        content_type: {:?},\n",
            asset.content_type
        ));
        generated.push_str(&format!(
            "        bytes: include_bytes!({:?}),\n",
            asset.source_path.display().to_string()
        ));
        generated.push_str("    },\n");
    }
    generated.push_str("];\n");
    generated
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}
