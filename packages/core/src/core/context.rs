use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;

use super::diagnostics::Diagnostics;
use super::errors::CoreError;
use super::registry::RegistryHandle;
use super::tree_node::{NodeId, NodeList};
use super::tree_store::TreeStore;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodecState {
    originals: HashMap<NodeId, String>,
}

impl CodecState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn remember_original(&mut self, decoded: NodeId, original: impl Into<String>) {
        self.originals.insert(decoded, original.into());
    }

    pub fn original_for(&self, decoded: NodeId) -> Option<&str> {
        self.originals.get(&decoded).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.originals.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    pub matching_nodes: NodeList,
    pub variables: HashMap<String, NodeList>,
    pub dont_auto_create: bool,
    pub diagnostics: Option<Rc<RefCell<Diagnostics>>>,
    pub user_data: Option<NonNull<c_void>>,
    pub source_filename: String,
    pub codec_state: Option<Box<CodecState>>,
    pub codec_registry: RegistryHandle,
    pub string_interpolation_enabled: bool,
    pub print_store: Option<NonNull<TreeStore>>,
}

impl Context {
    pub fn empty(codec_registry: RegistryHandle) -> Self {
        Self {
            matching_nodes: Vec::new(),
            variables: HashMap::new(),
            dont_auto_create: false,
            diagnostics: None,
            user_data: None,
            source_filename: String::new(),
            codec_state: None,
            codec_registry,
            string_interpolation_enabled: true,
            print_store: None,
        }
    }

    pub fn from_matching_nodes(codec_registry: RegistryHandle, nodes: NodeList) -> Self {
        Self {
            matching_nodes: nodes,
            ..Self::empty(codec_registry)
        }
    }

    pub fn with_diagnostics(mut self, diagnostics: Option<Rc<RefCell<Diagnostics>>>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    pub fn with_user_data(mut self, user_data: Option<NonNull<c_void>>) -> Self {
        self.user_data = user_data;
        self
    }

    pub fn get_user_data(&self) -> Option<NonNull<c_void>> {
        self.user_data
    }

    pub fn set_user_data(&mut self, user_data: Option<NonNull<c_void>>) {
        self.user_data = user_data;
    }

    pub fn ensure_codec_state(&mut self) -> &mut CodecState {
        self.codec_state
            .get_or_insert_with(|| Box::new(CodecState::new()))
            .as_mut()
    }

    pub fn append_matching_node(&mut self, node: NodeId) {
        self.matching_nodes.push(node);
    }

    pub fn single_readonly_child_context(&self, candidate: NodeId) -> Result<Self, CoreError> {
        let mut ctx = self.child_context(vec![candidate])?;
        ctx.dont_auto_create = true;
        Ok(ctx)
    }

    pub fn single_child_context(&self, candidate: NodeId) -> Result<Self, CoreError> {
        self.child_context(vec![candidate])
    }

    pub fn get_variable(&self, name: &str) -> Option<&NodeList> {
        self.variables.get(name)
    }

    pub fn set_variable(&mut self, name: impl Into<String>, value: NodeList) {
        self.variables.insert(name.into(), value);
    }

    pub fn child_context(&self, results: NodeList) -> Result<Self, CoreError> {
        let mut child = Self::from_matching_nodes(self.codec_registry.clone(), results);
        child.variables = self.variables.clone();
        child.dont_auto_create = self.dont_auto_create;
        child.diagnostics = self.diagnostics.clone();
        child.user_data = self.user_data;
        child.source_filename = self.source_filename.clone();
        child.codec_state = self.codec_state.clone();
        child.string_interpolation_enabled = self.string_interpolation_enabled;
        child.print_store = self.print_store;
        Ok(child)
    }

    pub fn current_print_store(&self) -> Option<&TreeStore> {
        self.print_store.map(|store| {
            // SAFETY: `print_store` is only set temporarily while a caller
            // holds an immutable borrow to the underlying `TreeStore`.
            unsafe { store.as_ref() }
        })
    }

    pub fn to_string_value(&self) -> Result<String, CoreError> {
        Ok(String::new())
    }

    pub fn deep_clone(&self) -> Result<Self, CoreError> {
        self.child_context(self.matching_nodes.clone())
    }

    pub fn read_only_clone(&self) -> Result<Self, CoreError> {
        let mut ctx = self.deep_clone()?;
        ctx.dont_auto_create = true;
        Ok(ctx)
    }

    pub fn writable_clone(&self) -> Result<Self, CoreError> {
        let mut ctx = self.deep_clone()?;
        ctx.dont_auto_create = false;
        Ok(ctx)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::empty(RegistryHandle::default())
    }
}
