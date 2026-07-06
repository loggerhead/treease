use crate::operators::TreeNode;

use super::graph_builder::{BuilderConfig, GraphLanguage, GraphModel, default_config};
use super::graph_builder_preorder::{Builder as GraphPreorderBuilder, GraphDelta};
use super::graph_fragment_index::GraphFragmentIndex;
use crate::analysis::document_analysis::StoredDocumentAnalysisOwned;
use crate::tree::tree_node::NodeId;
use crate::tree::tree_store::{DocumentAnalysis, TreeEntry, TreeStore};

#[derive(Debug, Clone, Copy)]
pub struct DocumentState<'a> {
    pub analysis: &'a TreeEntry,
    pub graph_model: Option<&'a GraphModel>,
    pub fragment_index: Option<&'a GraphFragmentIndex>,
}

pub struct AuthoritativeGraphService {
    builder: GraphPreorderBuilder,
    last_root: Option<TreeNode>,
    /// Cached current model, updated on each rebuild.
    current: GraphModel,
}

pub fn get_document_state<'a>(
    store: &'a TreeStore,
    document_key: &str,
) -> Option<DocumentState<'a>> {
    let analysis = store.get_tree_entry(document_key)?;
    Some(DocumentState {
        analysis,
        graph_model: store.get_graph(document_key),
        fragment_index: store.get_graph_index(document_key),
    })
}

pub fn get_authoritative_tree(store: &TreeStore, document_key: &str) -> Option<NodeId> {
    get_document_state(store, document_key).map(|state| state.analysis.root)
}

pub fn get_document_analysis<'a>(
    store: &'a TreeStore,
    document_key: &str,
) -> Option<DocumentAnalysis<'a>> {
    get_document_state(store, document_key)?;
    store.get_document_analysis(document_key)
}

pub fn store_authoritative_graph(
    store: &mut TreeStore,
    document_key: &str,
    model: GraphModel,
    fragment_index: Option<GraphFragmentIndex>,
) {
    store.set_graph_with_index(document_key, model, fragment_index);
}

pub fn store_owned_document_analysis(
    store: &mut TreeStore,
    document_key: &str,
    analysis: StoredDocumentAnalysisOwned,
) {
    let source = String::from_utf8_lossy(&analysis.source).into_owned();
    store.store_document_analysis_owned(
        document_key,
        &analysis.language,
        analysis.root,
        analysis.ts_tree,
        &source,
        analysis.token_spans,
        analysis.diagnostics_raw,
        analysis.semantic_tokens_encoded,
        analysis.value_json.unwrap_or_default(),
    );
}

pub fn clear_document_state(store: &mut TreeStore, document_key: &str) {
    store.remove_graph(document_key);
    store.remove_tree(document_key);
}

impl std::fmt::Debug for AuthoritativeGraphService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthoritativeGraphService").finish()
    }
}

impl AuthoritativeGraphService {
    pub fn new(language: GraphLanguage) -> Self {
        Self::with_config(language, default_config())
    }

    pub fn with_config(language: GraphLanguage, config: BuilderConfig) -> Self {
        Self {
            builder: GraphPreorderBuilder::new(config, language),
            last_root: None,
            current: GraphModel::default(),
        }
    }

    pub fn replace_document(&mut self, root: &TreeNode) -> GraphDelta {
        self.last_root = Some(root.clone());
        let delta = self.builder.build_from_tree(root).unwrap_or_default();
        // Cache the current model from the builder's internal state.
        self.current = GraphModel {
            nodes: self.builder.nodes().to_vec(),
            edges: self.builder.edges().to_vec(),
            ..Default::default()
        };
        self.current.rebuild_edge_index();
        delta
    }

    pub fn rebuild(&mut self) -> Option<GraphDelta> {
        let root = self.last_root.clone()?;
        Some(self.replace_document(&root))
    }

    pub fn current_model(&self) -> &GraphModel {
        &self.current
    }

    pub fn reset(&mut self) {
        self.last_root = None;
        self.current = GraphModel::default();
        self.builder.reset();
    }
}

pub fn graph_language_from_name(language: &str) -> GraphLanguage {
    match language {
        "json" => GraphLanguage::Json,
        "yaml" | "yml" => GraphLanguage::Yaml,
        "toml" => GraphLanguage::Toml,
        "" => GraphLanguage::None,
        _ => GraphLanguage::Unknown,
    }
}
