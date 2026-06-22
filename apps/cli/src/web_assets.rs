use std::env;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::CliError;

include!(concat!(env!("OUT_DIR"), "/treease_web_config.rs"));

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub(super) struct DiskAsset {
    pub path: PathBuf,
    pub content_type: &'static str,
}

#[derive(Debug, Deserialize)]
struct WebAssetManifest {
    version: String,
    files: Vec<WebAssetManifestFile>,
}

#[derive(Debug, Deserialize)]
struct WebAssetManifestFile {
    path: String,
    sha256: String,
    size: u64,
}

pub(super) fn ensure_available() -> Result<PathBuf, CliError> {
    let version_dir = version_cache_dir();
    if cache_is_complete(&version_dir) {
        return Ok(version_dir);
    }

    if version_dir.exists() {
        fs::remove_dir_all(&version_dir).map_err(CliError::Io)?;
    }

    download_version_into_cache(&version_dir)?;
    Ok(version_dir)
}

pub(super) fn find_asset(root_dir: &Path, request_path: &str) -> Option<DiskAsset> {
    let normalized = normalize_asset_path(request_path)?;
    let relative = normalized.strip_prefix('/')?;
    let relative_path = Path::new(relative);
    if relative_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }

    let asset_path = root_dir.join(relative_path);
    if !asset_path.is_file() {
        return None;
    }

    Some(DiskAsset {
        content_type: content_type_for(&asset_path),
        path: asset_path,
    })
}

fn download_version_into_cache(version_dir: &Path) -> Result<(), CliError> {
    let manifest_url = format!(
        "{}/{}/manifest.json",
        runtime_asset_base_url(),
        WEB_ASSET_VERSION
    );
    let client = http_client()?;
    let manifest_response = client
        .get(&manifest_url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| {
            CliError::WebAssetDownload(format!("failed to fetch {manifest_url}: {error}"))
        })?;
    let manifest_bytes = manifest_response.bytes().map_err(|error| {
        CliError::WebAssetDownload(format!("failed to read {manifest_url}: {error}"))
    })?;
    let manifest: WebAssetManifest = serde_json::from_slice(&manifest_bytes).map_err(|error| {
        CliError::WebAssetManifest(format!("invalid manifest at {manifest_url}: {error}"))
    })?;
    if manifest.version != WEB_ASSET_VERSION {
        return Err(CliError::WebAssetManifest(format!(
            "manifest version mismatch: expected {}, got {}",
            WEB_ASSET_VERSION, manifest.version
        )));
    }

    let cache_root = version_dir
        .parent()
        .ok_or_else(|| CliError::WebAssetCache("missing cache parent directory".to_string()))?;
    fs::create_dir_all(cache_root).map_err(CliError::Io)?;
    let temp_dir = temp_download_dir(cache_root);
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(CliError::Io)?;
    }
    fs::create_dir_all(&temp_dir).map_err(CliError::Io)?;

    let download_result = download_manifest_files(&client, &manifest, &temp_dir)
        .and_then(|_| write_manifest_copy(&temp_dir, &manifest_bytes));

    if let Err(error) = download_result {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(error);
    }

    if version_dir.exists() {
        fs::remove_dir_all(version_dir).map_err(CliError::Io)?;
    }
    fs::rename(&temp_dir, version_dir).map_err(CliError::Io)?;
    Ok(())
}

fn download_manifest_files(
    client: &Client,
    manifest: &WebAssetManifest,
    temp_dir: &Path,
) -> Result<(), CliError> {
    for file in &manifest.files {
        let relative_path = safe_relative_path(&file.path)?;
        let asset_url = format!(
            "{}/{}/{}",
            runtime_asset_base_url(),
            manifest.version,
            file.path
        );
        let response = client
            .get(&asset_url)
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| {
                CliError::WebAssetDownload(format!("failed to fetch {}: {error}", asset_url))
            })?;
        let bytes = response.bytes().map_err(|error| {
            CliError::WebAssetDownload(format!("failed to read {}: {error}", asset_url))
        })?;
        if bytes.len() as u64 != file.size {
            return Err(CliError::WebAssetManifest(format!(
                "asset size mismatch for {}: expected {}, got {}",
                file.path,
                file.size,
                bytes.len()
            )));
        }
        let actual_sha = sha256_hex(&bytes);
        if actual_sha != file.sha256 {
            return Err(CliError::WebAssetManifest(format!(
                "asset hash mismatch for {}: expected {}, got {}",
                file.path, file.sha256, actual_sha
            )));
        }
        let target_path = temp_dir.join(relative_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(CliError::Io)?;
        }
        fs::write(&target_path, &bytes).map_err(CliError::Io)?;
    }
    Ok(())
}

fn write_manifest_copy(temp_dir: &Path, manifest_bytes: &[u8]) -> Result<(), CliError> {
    fs::write(temp_dir.join("manifest.json"), manifest_bytes).map_err(CliError::Io)
}

fn cache_is_complete(version_dir: &Path) -> bool {
    let manifest_path = version_dir.join("manifest.json");
    let manifest_bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let manifest: WebAssetManifest = match serde_json::from_slice(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(_) => return false,
    };
    if manifest.version != WEB_ASSET_VERSION {
        return false;
    }
    manifest.files.iter().all(|file| {
        safe_relative_path(&file.path)
            .ok()
            .map(|relative_path| version_dir.join(relative_path).is_file())
            .unwrap_or(false)
    })
}

fn normalize_asset_path(request_path: &str) -> Option<String> {
    let path = request_path.split('?').next().unwrap_or("/");
    let path = if path.is_empty() { "/" } else { path };
    if path == "/" || path == "/cli/graph" {
        return Some("/index.html".to_string());
    }
    if path == "/cli/graph/" {
        return None;
    }
    Some(path.to_string())
}

fn safe_relative_path(value: &str) -> Result<PathBuf, CliError> {
    let path = Path::new(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CliError::WebAssetManifest(format!(
            "invalid asset path in manifest: {}",
            value
        )));
    }
    Ok(path.to_path_buf())
}

fn version_cache_dir() -> PathBuf {
    runtime_cache_root().join(WEB_ASSET_VERSION)
}

fn runtime_cache_root() -> PathBuf {
    if let Ok(path) = env::var("TREEASE_WEB_CACHE_DIR") {
        return PathBuf::from(path);
    }
    dirs::cache_dir()
        .unwrap_or_else(env::temp_dir)
        .join("treease")
        .join("web")
}

fn runtime_asset_base_url() -> String {
    env::var("TREEASE_WEB_ASSET_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_WEB_ASSET_BASE_URL.to_string())
}

fn temp_download_dir(cache_root: &Path) -> PathBuf {
    cache_root.join(format!(
        ".download-{}-{}",
        WEB_ASSET_VERSION,
        std::process::id()
    ))
}

fn http_client() -> Result<Client, CliError> {
    Client::builder()
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_READ_TIMEOUT)
        .user_agent(format!("treease-cli/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| {
            CliError::WebAssetDownload(format!("failed to initialize HTTP client: {error}"))
        })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(hex_char(byte >> 4));
        output.push(hex_char(byte & 0x0f));
    }
    output
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + value - 10) as char,
        _ => unreachable!("hex nibble should be in range 0..=15"),
    }
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
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

#[allow(dead_code)]
pub(super) fn read_asset_bytes(asset: &DiskAsset) -> io::Result<Vec<u8>> {
    fs::read(&asset.path)
}
