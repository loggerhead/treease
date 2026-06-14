use std::collections::HashMap;

use super::graph_fragment_index::GraphFragmentIndex;

pub fn compute_ancestor_relayout_chain(
    index: &GraphFragmentIndex,
    changed_stable_ids: &[u64],
) -> Vec<u64> {
    let mut ordered = Vec::new();
    let mut seen = HashMap::<u64, ()>::new();

    for changed_stable_id in changed_stable_ids {
        let mut cursor = Some(*changed_stable_id);
        while let Some(stable_id) = cursor {
            if seen.insert(stable_id, ()).is_none() {
                ordered.push(stable_id);
            }
            cursor = index
                .get_by_stable_id(stable_id)
                .and_then(|fragment| fragment.parent_stable_id);
        }
    }

    ordered
}
