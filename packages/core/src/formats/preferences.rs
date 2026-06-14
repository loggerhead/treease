use crate::core::FormatLanguage;

pub const DEFAULT_MAX_LINE_LENGTH: i32 = 100;
pub const DEFAULT_MAX_INLINE_COMPLEXITY: i32 = 1;
pub const DEFAULT_MAX_ARRAY_INLINE_ITEMS: i32 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatPreferences {
    pub indent: i32,
    pub unwrap_scalar: bool,
    pub smart: bool,
    pub max_line_length: i32,
    pub max_inline_complexity: i32,
    pub max_array_inline_items: i32,
    pub align_object_arrays: bool,
    pub leading_content_pre_processing: bool,
    pub print_doc_separators: bool,
    pub evaluate_together: bool,
    pub fix_merge_anchor_to_spec: bool,
    pub separator: char,
    pub auto_parse: bool,
}

impl FormatPreferences {
    pub fn base() -> Self {
        Self {
            indent: 2,
            unwrap_scalar: true,
            smart: true,
            max_line_length: DEFAULT_MAX_LINE_LENGTH,
            max_inline_complexity: DEFAULT_MAX_INLINE_COMPLEXITY,
            max_array_inline_items: DEFAULT_MAX_ARRAY_INLINE_ITEMS,
            align_object_arrays: true,
            leading_content_pre_processing: false,
            print_doc_separators: false,
            evaluate_together: false,
            fix_merge_anchor_to_spec: false,
            separator: '\0',
            auto_parse: false,
        }
    }
}

impl Default for FormatPreferences {
    fn default() -> Self {
        Self::base()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguagePreferences {
    pub default: FormatPreferences,
    pub json: FormatPreferences,
    pub yaml: FormatPreferences,
    pub toml: FormatPreferences,
    pub python: FormatPreferences,
    pub javascript: FormatPreferences,
    pub csv: FormatPreferences,
}

impl LanguagePreferences {
    pub fn effective(&self, language: FormatLanguage) -> FormatPreferences {
        let override_prefs = match language {
            FormatLanguage::Json => &self.json,
            FormatLanguage::Yaml => &self.yaml,
            FormatLanguage::Toml => &self.toml,
            FormatLanguage::Python => &self.python,
            FormatLanguage::Javascript => &self.javascript,
            FormatLanguage::Csv => &self.csv,
        };
        merge_preferences(&self.default, override_prefs)
    }

    pub fn apply(&self, language: FormatLanguage, override_prefs: FormatPreferences) -> Self {
        let mut out = self.clone();
        match language {
            FormatLanguage::Json => out.json = override_prefs,
            FormatLanguage::Yaml => out.yaml = override_prefs,
            FormatLanguage::Toml => out.toml = override_prefs,
            FormatLanguage::Python => out.python = override_prefs,
            FormatLanguage::Javascript => out.javascript = override_prefs,
            FormatLanguage::Csv => out.csv = override_prefs,
        }
        out
    }
}

pub fn default_language_preferences() -> LanguagePreferences {
    let base = FormatPreferences::base();

    let mut json = base.clone();
    json.unwrap_scalar = false;

    let mut yaml = base.clone();
    yaml.unwrap_scalar = false;
    yaml.leading_content_pre_processing = true;
    yaml.print_doc_separators = true;

    let mut python = base.clone();
    python.unwrap_scalar = false;

    let mut javascript = base.clone();
    javascript.unwrap_scalar = false;

    let mut csv = base.clone();
    csv.separator = ',';
    csv.auto_parse = true;

    LanguagePreferences {
        default: base.clone(),
        json,
        yaml,
        toml: base,
        python,
        javascript,
        csv,
    }
}

pub fn configured_language_preferences() -> LanguagePreferences {
    default_language_preferences()
}

fn merge_preferences(
    base: &FormatPreferences,
    override_prefs: &FormatPreferences,
) -> FormatPreferences {
    FormatPreferences {
        indent: override_prefs.indent,
        unwrap_scalar: override_prefs.unwrap_scalar,
        smart: override_prefs.smart,
        max_line_length: override_prefs.max_line_length,
        max_inline_complexity: override_prefs.max_inline_complexity,
        max_array_inline_items: override_prefs.max_array_inline_items,
        align_object_arrays: override_prefs.align_object_arrays,
        leading_content_pre_processing: override_prefs.leading_content_pre_processing,
        print_doc_separators: override_prefs.print_doc_separators,
        evaluate_together: override_prefs.evaluate_together,
        fix_merge_anchor_to_spec: override_prefs.fix_merge_anchor_to_spec,
        separator: override_prefs.separator,
        auto_parse: override_prefs.auto_parse,
        ..base.clone()
    }
}
