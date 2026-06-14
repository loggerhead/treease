use std::fs;
use std::path::{Path, PathBuf};

use treease_core::core::FormatLanguage;
use treease_core::formats::{
    Decode, Encode, JavascriptEncoder, JsonDecoder, PythonEncoder, TomlEncoder, YamlEncoder,
    default_language_preferences,
};

const TARGET_EXTENSIONS: &[&str] = &["yaml", "toml", "py", "js"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages directory")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn read_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(fs::read_to_string(path)?)
}

fn read_optional_file(path: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(Box::new(err)),
    }
}

fn ensure_trailing_newline(text: String) -> String {
    if text.ends_with('\n') {
        text
    } else {
        format!("{text}\n")
    }
}

fn encode_target(
    extension: &str,
    store: &treease_core::core::TreeStore,
    root: treease_core::core::NodeId,
) -> Result<String, Box<dyn std::error::Error>> {
    let prefs = default_language_preferences();
    let mut out = Vec::new();

    match extension {
        "yaml" => {
            YamlEncoder::new(prefs.effective(FormatLanguage::Yaml)).encode(store, root, &mut out)?
        }
        "toml" => {
            TomlEncoder::new(prefs.effective(FormatLanguage::Toml)).encode(store, root, &mut out)?
        }
        "py" => PythonEncoder::new(prefs.effective(FormatLanguage::Python))
            .encode(store, root, &mut out)?,
        "js" => JavascriptEncoder::new(prefs.effective(FormatLanguage::Javascript))
            .encode(store, root, &mut out)?,
        other => return Err(format!("unsupported target extension: {other}").into()),
    }

    Ok(String::from_utf8(out)?)
}

fn sync_target(
    example_dir: &Path,
    extension: &str,
    store: &treease_core::core::TreeStore,
    root: treease_core::core::NodeId,
) -> Result<(), Box<dyn std::error::Error>> {
    let override_path = example_dir.join(format!("simple.overrides.{extension}"));
    let output_path = example_dir.join(format!("simple.{extension}"));

    let final_content = if let Some(override_content) = read_optional_file(&override_path)? {
        ensure_trailing_newline(override_content)
    } else {
        ensure_trailing_newline(encode_target(extension, store, root)?)
    };

    fs::write(output_path, final_content)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = repo_root();
    let example_dir = repo_root.join("example");
    let json_source = read_file(&example_dir.join("simple.json"))?;
    let decoded = JsonDecoder.decode_str(&json_source)?;

    for extension in TARGET_EXTENSIONS {
        sync_target(&example_dir, extension, &decoded.store, decoded.root)?;
    }

    Ok(())
}
