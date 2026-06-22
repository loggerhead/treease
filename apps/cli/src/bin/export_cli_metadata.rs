use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("apps directory")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn write_json(path: PathBuf, value: &serde_json::Value) -> std::io::Result<()> {
    let mut output = serde_json::to_string_pretty(value).expect("metadata should serialize");
    output.push('\n');
    fs::write(path, output)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = repo_root().join("docs/generated");
    fs::create_dir_all(&out_dir)?;

    write_json(
        out_dir.join("cli-help.json"),
        &treease_cli::internal_metadata::cli_help_json(),
    )?;
    write_json(
        out_dir.join("operators.json"),
        &treease_cli::internal_metadata::operators_json(),
    )?;
    write_json(
        out_dir.join("formats.json"),
        &treease_cli::internal_metadata::formats_json(),
    )?;

    Ok(())
}
