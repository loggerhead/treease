use std::collections::HashMap;

use crate::formats::{Decode, Encode};

use super::errors::CoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatPreferences {
    pub indent: i32,
    pub pretty: bool,
}

impl Default for FormatPreferences {
    fn default() -> Self {
        Self {
            indent: 2,
            pretty: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatDefinition {
    pub name: String,
    pub encoder_symbol: Option<String>,
    pub decoder_symbol: Option<String>,
    pub encoder_prefs_symbol: Option<String>,
    pub decoder_prefs_symbol: Option<String>,
}

pub type EncoderFactory = fn(&FormatPreferences) -> Result<Box<dyn Encode>, CoreError>;
pub type DecoderFactory = fn(&FormatPreferences) -> Result<Box<dyn Decode>, CoreError>;

#[derive(Debug, Clone, Default)]
pub struct FormatRegistry {
    formats: HashMap<String, FormatDefinition>,
    encoder_factories: HashMap<String, EncoderFactory>,
    decoder_factories: HashMap<String, DecoderFactory>,
}

impl FormatRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init() -> Self {
        Self::new()
    }

    pub fn deinit(&mut self) {
        self.formats.clear();
    }

    pub fn register_format(&mut self, format: FormatDefinition) {
        self.formats.insert(format.name.clone(), format);
    }

    pub fn register_encoder_factory(&mut self, name: &str, factory: EncoderFactory) {
        self.encoder_factories.insert(name.to_owned(), factory);
    }

    pub fn register_decoder_factory(&mut self, name: &str, factory: DecoderFactory) {
        self.decoder_factories.insert(name.to_owned(), factory);
    }

    pub fn get(&self, name: &str) -> Option<&FormatDefinition> {
        self.formats.get(name)
    }

    pub fn get_encoder_by_prefs(&self, name: &str, _prefs: &FormatPreferences) -> Option<&str> {
        self.formats.get(name).and_then(|format| {
            format
                .encoder_prefs_symbol
                .as_deref()
                .or(format.encoder_symbol.as_deref())
        })
    }

    pub fn get_decoder_by_prefs(&self, name: &str, _prefs: &FormatPreferences) -> Option<&str> {
        self.formats.get(name).and_then(|format| {
            format
                .decoder_prefs_symbol
                .as_deref()
                .or(format.decoder_symbol.as_deref())
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &FormatDefinition> {
        self.formats.values()
    }

    pub fn create_encoder_by_prefs(
        &self,
        name: &str,
        prefs: &FormatPreferences,
    ) -> Option<Box<dyn Encode>> {
        self.encoder_factories
            .get(name)
            .and_then(|factory| factory(prefs).ok())
    }

    pub fn create_decoder_by_prefs(
        &self,
        name: &str,
        prefs: &FormatPreferences,
    ) -> Option<Box<dyn Decode>> {
        self.decoder_factories
            .get(name)
            .and_then(|factory| factory(prefs).ok())
    }
}
