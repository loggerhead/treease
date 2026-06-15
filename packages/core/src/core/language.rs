use super::graph_builder::GraphLanguage;
use super::lang_spec::FormatLanguage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Json,
    Yaml,
    Toml,
    Python,
    Javascript,
    Csv,
    None,
}

impl Language {
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "json" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "toml" => Self::Toml,
            "python" => Self::Python,
            "javascript" => Self::Javascript,
            "csv" => Self::Csv,
            _ => Self::None,
        }
    }

    pub fn as_name(self) -> Option<&'static str> {
        match self {
            Self::Json => Some("json"),
            Self::Yaml => Some("yaml"),
            Self::Toml => Some("toml"),
            Self::Python => Some("python"),
            Self::Javascript => Some("javascript"),
            Self::Csv => Some("csv"),
            Self::None => None,
        }
    }

    pub fn as_format_language(self) -> Option<FormatLanguage> {
        match self {
            Self::Json => Some(FormatLanguage::Json),
            Self::Yaml => Some(FormatLanguage::Yaml),
            Self::Toml => Some(FormatLanguage::Toml),
            Self::Python => Some(FormatLanguage::Python),
            Self::Javascript => Some(FormatLanguage::Javascript),
            Self::Csv => Some(FormatLanguage::Csv),
            Self::None => None,
        }
    }
}

impl From<FormatLanguage> for Language {
    fn from(fl: FormatLanguage) -> Self {
        match fl {
            FormatLanguage::Json => Self::Json,
            FormatLanguage::Yaml => Self::Yaml,
            FormatLanguage::Toml => Self::Toml,
            FormatLanguage::Python => Self::Python,
            FormatLanguage::Javascript => Self::Javascript,
            FormatLanguage::Csv => Self::Csv,
        }
    }
}

impl From<GraphLanguage> for Language {
    fn from(gl: GraphLanguage) -> Self {
        match gl {
            GraphLanguage::None | GraphLanguage::Unknown => Self::None,
            GraphLanguage::Json => Self::Json,
            GraphLanguage::Yaml => Self::Yaml,
            GraphLanguage::Toml => Self::Toml,
        }
    }
}
