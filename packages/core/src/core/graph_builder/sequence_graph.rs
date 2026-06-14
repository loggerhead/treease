use crate::operators::{NodeKind, TreeNode};

use super::SequencePresentation;
use super::shared::sequence_has_header_table;

pub(super) fn sequence_presentation(node: &TreeNode) -> SequencePresentation {
    if node.kind != NodeKind::Sequence {
        return SequencePresentation::HeaderlessTable;
    }
    if sequence_has_header_table(node) {
        SequencePresentation::HeaderTable
    } else {
        SequencePresentation::HeaderlessTable
    }
}
