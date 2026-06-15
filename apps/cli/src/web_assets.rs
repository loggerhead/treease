#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EmbeddedAsset {
    pub path: &'static str,
    pub content_type: &'static str,
    #[allow(dead_code)]
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/treease_web_assets.rs"));

#[allow(dead_code)]
pub(super) fn assets_available() -> bool {
    WEB_ASSETS_AVAILABLE && !WEB_ASSETS.is_empty()
}

#[allow(dead_code)]
pub(super) fn embedded_assets() -> &'static [EmbeddedAsset] {
    WEB_ASSETS
}

#[allow(dead_code)]
pub(super) fn find_asset<'a>(
    assets: &'a [EmbeddedAsset],
    request_path: &str,
) -> Option<&'a EmbeddedAsset> {
    let normalized = normalize_asset_path(request_path);
    assets
        .iter()
        .find(|asset| asset.path == normalized)
        .or_else(|| {
            if is_cli_graph_route(&normalized) {
                assets.iter().find(|asset| asset.path == "/index.html")
            } else {
                None
            }
        })
}

#[allow(dead_code)]
fn is_cli_graph_route(path: &str) -> bool {
    path == "/cli/graph"
}

#[allow(dead_code)]
fn normalize_asset_path(request_path: &str) -> String {
    let path = request_path.split('?').next().unwrap_or("/");
    let path = if path.is_empty() { "/" } else { path };
    if path == "/" {
        "/index.html".to_string()
    } else {
        path.to_string()
    }
}
