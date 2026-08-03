use super::layout_engine::LayoutEngine;
use crate::graph::graph_builder::{BuilderConfig, GraphLanguage, GraphModel, PathSeg};
use crate::graph::graph_materialize::materialize_into_current_model;
use crate::graph::graph_shape::NodeShapeBuilder;
use crate::graph::graph_topology::GraphTopology;
use crate::layout::layout_engine::LayoutState;
use crate::tree::tree_node::NodeId;
use crate::tree::tree_store::TreeStore;

#[derive(Debug, Clone)]
pub(crate) struct FullGraphBuild {
    pub(crate) model: GraphModel,
    pub(crate) topology: GraphTopology,
    pub(crate) topology_bytes: Vec<u8>,
    pub(crate) layout_state: LayoutState,
}

#[derive(Debug, Clone)]
pub(crate) struct FullLayoutAdapter {
    config: BuilderConfig,
    language: GraphLanguage,
}

impl FullLayoutAdapter {
    pub(crate) fn new(config: BuilderConfig, language: GraphLanguage) -> Self {
        Self { config, language }
    }

    pub(crate) fn build_model(
        &self,
        store: &TreeStore,
        root: NodeId,
        root_path: &[PathSeg],
    ) -> Result<GraphModel, String> {
        self.build_model_with_runtime_state(store, root, root_path)
            .map(|build| build.model)
    }

    pub(crate) fn build_model_with_runtime_state(
        &self,
        store: &TreeStore,
        root: NodeId,
        root_path: &[PathSeg],
    ) -> Result<FullGraphBuild, String> {
        let mut topology = GraphTopology::new();
        let (dirty, topology_bytes) = topology.build_full_with_topology_bytes(store, root, &self.config);
        rewrite_root_path(&mut topology, root_path);

        let mut model = GraphModel::default();
        let shape_builder = NodeShapeBuilder::new(&self.config, self.language);
        materialize_into_current_model(
            &mut topology,
            &mut model,
            store,
            &dirty,
            &shape_builder,
            &self.config,
        );
        let mut layout_state = LayoutState::default();
        if let Some(root_handle) = topology.root_handle() {
            LayoutEngine::new(self.config.clone()).layout_full_with_topology(
                &mut layout_state,
                &topology,
                &mut model,
                root_handle,
            );
        }
        Ok(FullGraphBuild {
            model,
            topology,
            topology_bytes,
            layout_state,
        })
    }
}

fn rewrite_root_path(topology: &mut GraphTopology, root_path: &[PathSeg]) {
    if root_path.is_empty() {
        return;
    }
    let handles: Vec<u32> = topology
        .slots()
        .iter()
        .enumerate()
        .map(|(handle, _)| handle as u32)
        .collect();
    for handle in handles {
        let Some(slot) = topology.slot_mut(handle) else {
            continue;
        };
        let mut next_path = root_path.to_vec();
        next_path.extend_from_slice(&slot.path);
        slot.path = next_path;
        slot.depth = slot.path.len() as u32;
    }
}
