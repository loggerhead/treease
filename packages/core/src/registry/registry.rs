use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use super::format_registry::{FormatPreferences, FormatRegistry};
use super::operator_registry::OperatorRegistry;
use crate::io::codec_service::CodecService;

#[derive(Debug, Clone, Default)]
pub struct Registry {
    pub operators: OperatorRegistry,
    pub formats: FormatRegistry,
    pub codecs: CodecService,
}

impl Registry {
    pub fn new(operators: OperatorRegistry, formats: FormatRegistry) -> Self {
        Self {
            operators,
            formats,
            codecs: CodecService::new(),
        }
    }

    /// Create an empty registry (no handlers populated).
    /// The operator registry stores `OperatorHandler` function pointers
    /// alongside symbol names, ready for dispatch via `get_handler()`.
    pub fn init() -> Self {
        Self::new(OperatorRegistry::init(), FormatRegistry::init())
    }

    /// Ensure the global singleton registry is initialized.
    /// Returns a RegistryHandle that can be used in Context.
    /// The registry is populated lazily on first access.
    pub fn ensure_global() -> RegistryHandle {
        thread_local! {
            static GLOBAL: OnceCell<RegistryHandle> = const { OnceCell::new() };
        }
        GLOBAL.with(|global| global.get_or_init(|| to_handle(Registry::init())).clone())
    }

    pub fn get_encoder(&self, format: &str, prefs: &FormatPreferences) -> Option<&str> {
        self.formats.get_encoder_by_prefs(format, prefs)
    }

    pub fn get_decoder(&self, format: &str, prefs: &FormatPreferences) -> Option<&str> {
        self.formats.get_decoder_by_prefs(format, prefs)
    }
}

#[derive(Debug, Clone)]
pub struct RegistryHandle {
    pub inner: Rc<RefCell<Registry>>,
}

impl Default for RegistryHandle {
    fn default() -> Self {
        to_handle(Registry::default())
    }
}

#[derive(Debug, Clone)]
pub struct RegistryOwner {
    registry: RegistryHandle,
    pub owns: bool,
}

impl RegistryOwner {
    pub fn init_owned() -> Self {
        Self {
            registry: to_handle(Registry::init()),
            owns: true,
        }
    }

    pub fn init_borrowed(handle: RegistryHandle) -> Self {
        Self {
            registry: handle,
            owns: false,
        }
    }

    pub fn ptr(&self) -> RegistryHandle {
        self.registry.clone()
    }

    pub fn handle(&self) -> RegistryHandle {
        self.registry.clone()
    }

    pub fn deinit(&mut self) {
        self.registry = RegistryHandle::default();
        self.owns = false;
    }
}

pub fn to_handle(registry: Registry) -> RegistryHandle {
    RegistryHandle {
        inner: Rc::new(RefCell::new(registry)),
    }
}

pub fn from_handle(handle: &RegistryHandle) -> Rc<RefCell<Registry>> {
    handle.inner.clone()
}
