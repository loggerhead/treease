use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=TREEASE_WEB_URL");

    let web_url =
        env::var("TREEASE_WEB_URL").unwrap_or_else(|_| "https://treease.com/editor".to_string());

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR should be set"));
    let generated_path = out_dir.join("treease_web_config.rs");
    fs::write(
        generated_path,
        format!("pub(super) const DEFAULT_WEB_URL: &str = {:?};\n", web_url),
    )
    .expect("generated web config should be writable");
}
