use super::errors::{CoreError, FormatError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    pub formal_name: &'static str,
    pub names: &'static [&'static str],
    pub has_encoder: bool,
    pub has_decoder: bool,
}

impl Format {
    pub const fn new(
        formal_name: &'static str,
        names: &'static [&'static str],
        has_encoder: bool,
        has_decoder: bool,
    ) -> Self {
        Self {
            formal_name,
            names,
            has_encoder,
            has_decoder,
        }
    }

    pub fn matches_name(&self, name: &str) -> bool {
        self.formal_name == name || self.names.contains(&name)
    }
}

pub const YAML_FORMAT: Format = Format::new("yaml", &["y", "yml"], true, true);
pub const JSON_FORMAT: Format = Format::new("json", &["j"], true, true);
pub const CSV_FORMAT: Format = Format::new("csv", &["c"], true, true);
pub const URI_FORMAT: Format = Format::new("uri", &[], true, true);
pub const TOML_FORMAT: Format = Format::new("toml", &[], true, true);
pub const PYTHON_FORMAT: Format = Format::new("python", &["py"], true, true);
pub const JAVASCRIPT_FORMAT: Format = Format::new("javascript", &["js"], true, true);

pub const FORMATS: &[Format] = &[
    YAML_FORMAT,
    JSON_FORMAT,
    CSV_FORMAT,
    URI_FORMAT,
    TOML_FORMAT,
    PYTHON_FORMAT,
    JAVASCRIPT_FORMAT,
];

pub fn format_string_from_filename(filename: &str) -> &str {
    if filename.is_empty() {
        return "json";
    }

    let last_segment = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let Some((_, extension)) = last_segment.rsplit_once('.') else {
        return "json";
    };

    let extension_lower = extension.to_ascii_lowercase();
    match extension_lower.as_str() {
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "csv" => "csv",
        "toml" => "toml",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        _ => extension,
    }
}

pub fn format_from_string(format: &str) -> Result<&'static Format, CoreError> {
    if !format.is_empty() {
        for candidate in FORMATS {
            if candidate.matches_name(format) {
                return Ok(candidate);
            }
        }
    }
    Err(FormatError::UnknownFormat.into())
}
