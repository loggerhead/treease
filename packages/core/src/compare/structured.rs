use crate::compare::{
    Diff, DiffOptions, DiffPair, DiffType, array_diff, classify, myers_diff_with_options, new_diff,
};
use crate::core::core_helpers::recursive_node_compare;
use crate::formats::DecodedDocument;
use crate::operators::{NodeKind, SemType, TreeNode};
use std::collections::{HashMap, HashSet};

const INLINE_MAX_EDIT_LENGTH: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StructuredNodeType {
    Object,
    Array,
    String,
    Number,
    Boolean,
    Null,
    Unknown,
}

fn decode_to_compat_tree(language: &str, text: &str) -> Result<TreeNode, String> {
    let decoded: DecodedDocument = crate::stream::decode_to_document(language, text)
        .map_err(|e| format!("{language} decode: {e}"))?;
    crate::operators::registry_tables_formats::core_tree_to_compat(&decoded.store, decoded.root)
        .map_err(|e| format!("core tree conversion: {e:?}"))
}

fn node_type(node: &TreeNode) -> StructuredNodeType {
    match node.kind {
        NodeKind::Mapping => StructuredNodeType::Object,
        NodeKind::Sequence => StructuredNodeType::Array,
        NodeKind::Alias => StructuredNodeType::Unknown,
        NodeKind::Scalar => match node.resolved_sem_type() {
            Some(SemType::Str) => StructuredNodeType::String,
            Some(SemType::Int) | Some(SemType::Float) => StructuredNodeType::Number,
            Some(SemType::Boolean) => StructuredNodeType::Boolean,
            Some(SemType::Nil) => StructuredNodeType::Null,
            Some(SemType::Map) => StructuredNodeType::Object,
            Some(SemType::Seq) => StructuredNodeType::Array,
            None => StructuredNodeType::Unknown,
        },
        NodeKind::Unknown => StructuredNodeType::Unknown,
    }
}

fn raw_text<'a>(node: &TreeNode, source: &'a str) -> &'a str {
    source
        .get(node.start_byte as usize..node.end_byte as usize)
        .unwrap_or("")
}

fn raw_diff(node: &TreeNode, diff_type: DiffType) -> Diff {
    new_diff(
        i32::try_from(node.start_byte).unwrap_or(0),
        i32::try_from(node.end_byte.saturating_sub(node.start_byte)).unwrap_or(0),
        diff_type,
    )
}

fn object_entry_diff(key: &TreeNode, value: &TreeNode, diff_type: DiffType) -> Diff {
    new_diff(
        i32::try_from(key.start_byte).unwrap_or(0),
        i32::try_from(value.end_byte.saturating_sub(key.start_byte)).unwrap_or(0),
        diff_type,
    )
}

fn mapping_entries(node: &TreeNode) -> Vec<(&TreeNode, &TreeNode)> {
    let mut entries = Vec::new();
    let mut index = 0;
    while index + 1 < node.content.len() {
        entries.push((&node.content[index], &node.content[index + 1]));
        index += 2;
    }
    entries
}

fn compare_inline_texts(left: &str, right: &str) -> Vec<Diff> {
    myers_diff_with_options(
        left,
        right,
        DiffOptions {
            is_array_diff: false,
            max_edit_length: INLINE_MAX_EDIT_LENGTH,
        },
    )
}

fn compare_array_tokens<'a>(left: &[&'a str], right: &[&'a str]) -> Vec<Diff> {
    array_diff(left, right, DiffOptions::default())
}

fn shifted_inline_diffs(node: &TreeNode, inline_diffs: Vec<Diff>) -> Vec<Diff> {
    let offset = i32::try_from(node.start_byte).unwrap_or(0);
    inline_diffs
        .into_iter()
        .map(|mut diff| {
            diff.offset += offset;
            diff
        })
        .collect()
}

struct StructuredComparer<'a> {
    left_source: &'a str,
    right_source: &'a str,
    pairs: Vec<DiffPair>,
}

impl<'a> StructuredComparer<'a> {
    fn new(left_source: &'a str, right_source: &'a str) -> Self {
        Self {
            left_source,
            right_source,
            pairs: Vec::new(),
        }
    }

    fn compare(mut self, left: &TreeNode, right: &TreeNode) -> Vec<DiffPair> {
        self.diff(left, right);
        self.pairs
    }

    fn diff(&mut self, left: &TreeNode, right: &TreeNode) {
        let left_type = node_type(left);
        let right_type = node_type(right);

        if left_type != right_type {
            self.pairs.push(DiffPair {
                left: Some(raw_diff(left, DiffType::Delete)),
                right: Some(raw_diff(right, DiffType::Insert)),
            });
            return;
        }

        match left_type {
            StructuredNodeType::Array => self.diff_array(left, right),
            StructuredNodeType::Object => self.diff_object(left, right),
            StructuredNodeType::String | StructuredNodeType::Number => {
                let left_inline = raw_text(left, self.left_source);
                let right_inline = raw_text(right, self.right_source);
                let classified = classify(&compare_inline_texts(left_inline, right_inline));
                if !classified.left.is_empty() || !classified.right.is_empty() {
                    let mut left_diff = raw_diff(left, DiffType::Delete);
                    let mut right_diff = raw_diff(right, DiffType::Insert);
                    left_diff.inline_diffs = shifted_inline_diffs(left, classified.left);
                    right_diff.inline_diffs = shifted_inline_diffs(right, classified.right);
                    self.pairs.push(DiffPair {
                        left: Some(left_diff),
                        right: Some(right_diff),
                    });
                }
            }
            StructuredNodeType::Boolean
            | StructuredNodeType::Null
            | StructuredNodeType::Unknown => {
                if left.value != right.value {
                    self.pairs.push(DiffPair {
                        left: Some(raw_diff(left, DiffType::Delete)),
                        right: Some(raw_diff(right, DiffType::Insert)),
                    });
                }
            }
        }
    }

    fn diff_array(&mut self, left: &TreeNode, right: &TreeNode) {
        let left_tokens = left
            .content
            .iter()
            .map(|child| raw_text(child, self.left_source))
            .collect::<Vec<_>>();
        let right_tokens = right
            .content
            .iter()
            .map(|child| raw_text(child, self.right_source))
            .collect::<Vec<_>>();
        let classified = classify(&compare_array_tokens(&left_tokens, &right_tokens));
        let pair_count = classified.left.len().max(classified.right.len());

        for index in 0..pair_count {
            let left_diff = classified.left.get(index);
            let right_diff = classified.right.get(index);
            let left_index = left_diff.and_then(|diff| usize::try_from(diff.offset).ok());
            let right_index = right_diff.and_then(|diff| usize::try_from(diff.offset).ok());

            if let (Some(li), Some(ri)) = (left_index, right_index) {
                if li == ri {
                    if let (Some(left_child), Some(right_child)) =
                        (left.content.get(li), right.content.get(ri))
                    {
                        self.diff(left_child, right_child);
                        continue;
                    }
                }
            }

            self.pairs.push(DiffPair {
                left: left_index
                    .and_then(|candidate| left.content.get(candidate))
                    .map(|node| raw_diff(node, DiffType::Delete)),
                right: right_index
                    .and_then(|candidate| right.content.get(candidate))
                    .map(|node| raw_diff(node, DiffType::Insert)),
            });
        }
    }

    fn diff_object(&mut self, left: &TreeNode, right: &TreeNode) {
        let left_entries = mapping_entries(left);
        let right_entries = mapping_entries(right);
        let mut right_by_key = HashMap::new();
        for (index, (key, value)) in right_entries.iter().enumerate() {
            right_by_key.insert(key.value.as_str(), (index, *key, *value));
        }

        let mut matched_right = HashSet::new();
        let mut left_only = Vec::new();

        for (left_key, left_value) in &left_entries {
            if let Some((right_index, _, right_value)) = right_by_key.get(left_key.value.as_str()) {
                matched_right.insert(*right_index);
                self.diff(left_value, right_value);
            } else {
                left_only.push((*left_key, *left_value));
            }
        }

        for (left_key, left_value) in left_only {
            self.pairs.push(DiffPair {
                left: Some(object_entry_diff(left_key, left_value, DiffType::Delete)),
                right: None,
            });
        }

        for (index, (right_key, right_value)) in right_entries.iter().enumerate() {
            if matched_right.contains(&index) {
                continue;
            }
            self.pairs.push(DiffPair {
                left: None,
                right: Some(object_entry_diff(right_key, right_value, DiffType::Insert)),
            });
        }
    }
}

/// Compare two texts structurally by parsing both into trees and comparing
/// the resulting tree nodes recursively.
///
/// Unlike the simple format-and-compare approach, this handles:
/// - Map key reordering (order-insensitive mapping comparison)
/// - Structural type mismatches (scalar vs map, etc.)
/// - Nil equality
///
/// Returns `Ok(true)` if the two texts are structurally equivalent,
/// `Ok(false)` if they differ, or `Err` if either text cannot be parsed.
pub fn compare_texts_structured(language: &str, left: &str, right: &str) -> Result<bool, String> {
    let left_root = decode_to_compat_tree(language, left)?;
    let right_root = decode_to_compat_tree(language, right)?;
    Ok(recursive_node_compare(&left_root, &right_root))
}

pub fn diff_texts_structured(
    language: &str,
    left: &str,
    right: &str,
) -> Result<Vec<DiffPair>, String> {
    let left_root = decode_to_compat_tree(language, left)?;
    let right_root = decode_to_compat_tree(language, right)?;
    Ok(StructuredComparer::new(left, right).compare(&left_root, &right_root))
}
