use std::io::Write;

use crate::errors::{CoreError, FormatError, ParseError};
use crate::tree::{NodeId, TreeNodeKind, TreeStore, ValueRep};

use super::preferences::FormatPreferences;
use super::{Encode, node};

const TIMESTAMP_TAG: &str = "!!timestamp";

#[derive(Debug, Clone, Default)]
pub struct TomlEncoder {
    pub prefs: FormatPreferences,
}

impl TomlEncoder {
    pub fn new(prefs: FormatPreferences) -> Self {
        Self { prefs }
    }
}

impl Encode for TomlEncoder {
    fn encode(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let root = node(store, node_id)?;
        if root.kind == TreeNodeKind::Scalar {
            writer.write_all(format_scalar(store, node_id)?.as_bytes())?;
            if self.prefs.indent > 0 {
                writer.write_all(b"\n")?;
            }
            return Ok(());
        }
        if root.kind != TreeNodeKind::Mapping {
            return Err(CoreError::Format(FormatError::TomlRequiresMap));
        }
        let mut out = String::new();
        let mut wrote_root_attr = false;
        self.encode_root_mapping(store, node_id, &mut out, &mut wrote_root_attr)?;
        writer.write_all(out.as_bytes())?;
        Ok(())
    }
}

impl TomlEncoder {
    fn is_minify(&self) -> bool {
        self.prefs.indent <= 0
    }

    fn eq_token(&self) -> &'static str {
        if self.is_minify() { "=" } else { " = " }
    }

    fn max_line_length(&self) -> usize {
        self.prefs.max_line_length.max(0) as usize
    }

    fn encode_root_mapping(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        out: &mut String,
        wrote_root_attr: &mut bool,
    ) -> Result<(), CoreError> {
        let root = node(store, node_id)?;
        for pair in root.content.chunks_exact(2) {
            let value = node(store, pair[1])?;
            if is_root_attribute_value(store, pair[1], value)? {
                self.encode_top_level_entry(
                    store,
                    &[store.value_for(pair[0])?],
                    pair[1],
                    out,
                    wrote_root_attr,
                )?;
            }
        }
        for pair in root.content.chunks_exact(2) {
            let value = node(store, pair[1])?;
            if value.kind == TreeNodeKind::Mapping && !value.content.is_empty() {
                self.encode_separate_mapping(
                    store,
                    &[store.value_for(pair[0])?],
                    pair[1],
                    out,
                    wrote_root_attr,
                )?;
            }
        }
        for pair in root.content.chunks_exact(2) {
            let value = node(store, pair[1])?;
            if value.kind == TreeNodeKind::Sequence
                && is_root_structural_value(store, pair[1], value)?
            {
                self.encode_top_level_entry(
                    store,
                    &[store.value_for(pair[0])?],
                    pair[1],
                    out,
                    wrote_root_attr,
                )?;
            }
        }
        Ok(())
    }

    fn encode_top_level_entry(
        &self,
        store: &TreeStore,
        path: &[&str],
        node_id: NodeId,
        out: &mut String,
        wrote_root_attr: &mut bool,
    ) -> Result<(), CoreError> {
        let key = *path
            .last()
            .ok_or(CoreError::Parse(ParseError::UnsupportedTomlValue))?;
        let current = node(store, node_id)?;
        match current.kind {
            TreeNodeKind::Scalar => {
                if is_nil_scalar(store, node_id)? {
                    return Err(CoreError::Parse(ParseError::UnsupportedTomlValue));
                }
                self.write_attribute(store, key, node_id, out, wrote_root_attr)
            }
            TreeNodeKind::Sequence => {
                if current.content.is_empty() {
                    return self.write_array_attribute(store, key, node_id, out, wrote_root_attr);
                }
                if all_maps(store, current)? {
                    self.write_array_tables(
                        store,
                        path,
                        node_id,
                        out,
                        !self.is_minify() && *wrote_root_attr,
                    )?;
                    return Ok(());
                }
                self.write_array_attribute(store, key, node_id, out, wrote_root_attr)
            }
            TreeNodeKind::Mapping => {
                if !should_encode_as_separate_mapping(store, current)? {
                    return self.write_inline_table_attribute(
                        store,
                        key,
                        node_id,
                        out,
                        wrote_root_attr,
                    );
                }
                self.encode_separate_mapping(store, path, node_id, out, wrote_root_attr)
            }
            TreeNodeKind::Alias | TreeNodeKind::Unknown => {
                Err(CoreError::Parse(ParseError::UnsupportedTomlValue))
            }
        }
    }

    fn write_table_header(&self, path: &[&str], out: &mut String, wrote_root_attr: &mut bool) {
        if !self.is_minify() && *wrote_root_attr {
            out.push('\n');
            *wrote_root_attr = false;
        }
        out.push('[');
        write_path(path, out);
        out.push_str("]\n");
    }

    fn encode_separate_mapping(
        &self,
        store: &TreeStore,
        path: &[&str],
        node_id: NodeId,
        out: &mut String,
        wrote_root_attr: &mut bool,
    ) -> Result<(), CoreError> {
        let map = node(store, node_id)?;
        if has_attributes(store, map)? || map.content.is_empty() {
            self.write_table_header(path, out, wrote_root_attr);
            return self.encode_mapping_body_with_path(store, path, node_id, out, wrote_root_attr);
        }

        for pair in map.content.chunks_exact(2) {
            let value = node(store, pair[1])?;
            let key = store.value_for(pair[0])?;
            match value.kind {
                TreeNodeKind::Mapping => {
                    if !should_encode_as_separate_mapping(store, value)? {
                        self.write_inline_table_attribute(
                            store,
                            key,
                            pair[1],
                            out,
                            wrote_root_attr,
                        )?;
                        continue;
                    }
                    let mut child_path = path.to_vec();
                    child_path.push(key);
                    self.write_table_header(&child_path, out, wrote_root_attr);
                    self.encode_mapping_body_with_path(
                        store,
                        &child_path,
                        pair[1],
                        out,
                        wrote_root_attr,
                    )?;
                }
                TreeNodeKind::Sequence => {
                    if !value.content.is_empty() && all_maps(store, value)? {
                        let mut child_path = path.to_vec();
                        child_path.push(key);
                        self.write_array_tables(store, &child_path, pair[1], out, false)?;
                    } else {
                        self.write_array_attribute(store, key, pair[1], out, wrote_root_attr)?;
                    }
                }
                TreeNodeKind::Scalar => {
                    if is_nil_scalar(store, pair[1])? {
                        continue;
                    }
                    self.write_attribute(store, key, pair[1], out, wrote_root_attr)?;
                }
                TreeNodeKind::Alias | TreeNodeKind::Unknown => {}
            }
        }

        Ok(())
    }

    fn encode_mapping_body_with_path(
        &self,
        store: &TreeStore,
        path: &[&str],
        node_id: NodeId,
        out: &mut String,
        wrote_root_attr: &mut bool,
    ) -> Result<(), CoreError> {
        let map = node(store, node_id)?;
        for pair in map.content.chunks_exact(2) {
            let value = node(store, pair[1])?;
            let key = store.value_for(pair[0])?;
            match value.kind {
                TreeNodeKind::Scalar => {
                    if is_nil_scalar(store, pair[1])? {
                        continue;
                    }
                    self.write_attribute(store, key, pair[1], out, wrote_root_attr)?;
                }
                TreeNodeKind::Sequence => {
                    if !all_maps(store, value)? {
                        self.write_array_attribute(store, key, pair[1], out, wrote_root_attr)?;
                    }
                }
                TreeNodeKind::Mapping | TreeNodeKind::Alias | TreeNodeKind::Unknown => {}
            }
        }

        let mut need_blank_before_first_array_table =
            !self.is_minify() && has_attributes(store, map)?;
        for pair in map.content.chunks_exact(2) {
            let key = store.value_for(pair[0])?;
            let value = node(store, pair[1])?;
            if value.kind == TreeNodeKind::Sequence
                && !value.content.is_empty()
                && all_maps(store, value)?
            {
                let mut child_path = path.to_vec();
                child_path.push(key);
                self.write_array_tables(
                    store,
                    &child_path,
                    pair[1],
                    out,
                    need_blank_before_first_array_table,
                )?;
                need_blank_before_first_array_table = false;
            }
        }

        for pair in map.content.chunks_exact(2) {
            let key = store.value_for(pair[0])?;
            let value = node(store, pair[1])?;
            if value.kind == TreeNodeKind::Mapping
                && !should_encode_as_separate_mapping(store, value)?
            {
                self.write_inline_table_attribute(store, key, pair[1], out, wrote_root_attr)?;
            }
        }

        for pair in map.content.chunks_exact(2) {
            let key = store.value_for(pair[0])?;
            let value = node(store, pair[1])?;
            if value.kind == TreeNodeKind::Mapping
                && should_encode_as_separate_mapping(store, value)?
            {
                let mut child_path = path.to_vec();
                child_path.push(key);
                self.write_table_header(&child_path, out, wrote_root_attr);
                self.encode_mapping_body_with_path(
                    store,
                    &child_path,
                    pair[1],
                    out,
                    wrote_root_attr,
                )?;
            }
        }

        Ok(())
    }

    fn write_array_tables(
        &self,
        store: &TreeStore,
        path: &[&str],
        node_id: NodeId,
        out: &mut String,
        need_blank_before_first: bool,
    ) -> Result<(), CoreError> {
        let seq = node(store, node_id)?;
        let mut need_blank_before_first = need_blank_before_first;
        for (index, item) in seq.content.iter().enumerate() {
            if !self.is_minify() && (index > 0 || need_blank_before_first) {
                out.push('\n');
            }
            need_blank_before_first = false;
            out.push_str("[[");
            write_path(path, out);
            out.push_str("]]\n");
            let mut nested_wrote_root_attr = false;
            self.encode_mapping_body_with_path(
                store,
                path,
                *item,
                out,
                &mut nested_wrote_root_attr,
            )?;
        }
        Ok(())
    }

    fn write_attribute(
        &self,
        store: &TreeStore,
        key: &str,
        value_id: NodeId,
        out: &mut String,
        wrote_root_attr: &mut bool,
    ) -> Result<(), CoreError> {
        *wrote_root_attr = true;
        write_key(key, out);
        out.push_str(self.eq_token());
        out.push_str(&format_scalar(store, value_id)?);
        out.push('\n');
        if self.is_minify() {
            *wrote_root_attr = false;
        }
        Ok(())
    }

    fn write_array_attribute(
        &self,
        store: &TreeStore,
        key: &str,
        value_id: NodeId,
        out: &mut String,
        wrote_root_attr: &mut bool,
    ) -> Result<(), CoreError> {
        *wrote_root_attr = true;
        let seq = node(store, value_id)?;
        write_key(key, out);
        out.push_str(self.eq_token());
        if seq.content.is_empty() {
            out.push_str("[]");
            out.push('\n');
            if self.is_minify() {
                *wrote_root_attr = false;
            }
            return Ok(());
        }

        let arr = self.sequence_to_inline_array(store, value_id)?;
        if self.should_write_multiline_array(key, &arr) {
            self.write_multiline_array(store, value_id, out)?;
            out.push('\n');
            return Ok(());
        }

        out.push_str(&arr);
        out.push('\n');
        if self.is_minify() {
            *wrote_root_attr = false;
        }
        Ok(())
    }

    fn write_inline_table_attribute(
        &self,
        store: &TreeStore,
        key: &str,
        value_id: NodeId,
        out: &mut String,
        wrote_root_attr: &mut bool,
    ) -> Result<(), CoreError> {
        *wrote_root_attr = true;
        write_key(key, out);
        out.push_str(self.eq_token());
        out.push_str(&self.mapping_to_inline_table(store, value_id)?);
        out.push('\n');
        if self.is_minify() {
            *wrote_root_attr = false;
        }
        Ok(())
    }

    fn should_write_multiline_array(&self, key: &str, arr: &str) -> bool {
        if self.is_minify() {
            return false;
        }
        key.len() + self.eq_token().len() + arr.len() > self.max_line_length()
    }

    fn write_multiline_array(
        &self,
        store: &TreeStore,
        value_id: NodeId,
        out: &mut String,
    ) -> Result<(), CoreError> {
        let seq = node(store, value_id)?;
        out.push_str("[\n");
        for (index, child) in seq.content.iter().enumerate() {
            out.push_str(&" ".repeat(self.prefs.indent.max(0) as usize));
            out.push_str(&self.value_to_inline_string(store, *child)?);
            if index + 1 != seq.content.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push(']');
        Ok(())
    }

    fn value_to_inline_string(
        &self,
        store: &TreeStore,
        node_id: NodeId,
    ) -> Result<String, CoreError> {
        let current = node(store, node_id)?;
        match current.kind {
            TreeNodeKind::Scalar => {
                if is_nil_scalar(store, node_id)? {
                    return Err(CoreError::Parse(ParseError::UnsupportedTomlValue));
                }
                format_scalar(store, node_id)
            }
            TreeNodeKind::Sequence => self.sequence_to_inline_array(store, node_id),
            TreeNodeKind::Mapping => self.mapping_to_inline_table(store, node_id),
            TreeNodeKind::Alias | TreeNodeKind::Unknown => {
                Err(CoreError::Parse(ParseError::UnsupportedTomlValue))
            }
        }
    }

    fn sequence_to_inline_array(
        &self,
        store: &TreeStore,
        node_id: NodeId,
    ) -> Result<String, CoreError> {
        let current = node(store, node_id)?;
        let mut items = Vec::with_capacity(current.content.len());
        for child in &current.content {
            items.push(self.value_to_inline_string(store, *child)?);
        }
        if self.is_minify() {
            Ok(format!("[{}]", items.join(",")))
        } else {
            Ok(format!("[{}]", items.join(", ")))
        }
    }

    fn mapping_to_inline_table(
        &self,
        store: &TreeStore,
        node_id: NodeId,
    ) -> Result<String, CoreError> {
        let current = node(store, node_id)?;
        let mut fields = Vec::new();
        for pair in current.content.chunks_exact(2) {
            let key = store.value_for(pair[0])?;
            let value = node(store, pair[1])?;
            let rhs = match value.kind {
                TreeNodeKind::Scalar => {
                    if is_nil_scalar(store, pair[1])? {
                        continue;
                    }
                    format_scalar(store, pair[1])?
                }
                TreeNodeKind::Sequence => self.sequence_to_inline_array(store, pair[1])?,
                TreeNodeKind::Mapping => {
                    if should_encode_as_separate_mapping(store, value)? {
                        continue;
                    }
                    self.mapping_to_inline_table(store, pair[1])?
                }
                TreeNodeKind::Alias | TreeNodeKind::Unknown => {
                    return Err(CoreError::Parse(ParseError::UnsupportedTomlValue));
                }
            };
            if self.is_minify() {
                fields.push(format!("{}={}", format_key(key), rhs));
            } else {
                fields.push(format!("{} = {}", format_key(key), rhs));
            }
        }
        if fields.is_empty() {
            return Ok("{}".to_string());
        }
        if self.is_minify() {
            Ok(format!("{{{}}}", fields.join(",")))
        } else {
            Ok(format!("{{ {} }}", fields.join(", ")))
        }
    }
}

fn is_nil_scalar(store: &TreeStore, node_id: NodeId) -> Result<bool, CoreError> {
    let node = node(store, node_id)?;
    if node.kind != TreeNodeKind::Scalar {
        return Ok(false);
    }
    let raw = store.value_for(node_id)?;
    Ok(node.get_value_rep_with(raw)? == ValueRep::Nil)
}

fn should_encode_as_separate_mapping(
    store: &TreeStore,
    mapping: &crate::tree::TreeNode,
) -> Result<bool, CoreError> {
    Ok(mapping.encode_separate()
        || has_encode_separate_child(store, mapping)?
        || has_structural_children(store, mapping)?)
}

fn has_encode_separate_child(
    store: &TreeStore,
    mapping: &crate::tree::TreeNode,
) -> Result<bool, CoreError> {
    for pair in mapping.content.chunks_exact(2) {
        let value = node(store, pair[1])?;
        if value.kind == TreeNodeKind::Mapping && value.encode_separate() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn has_structural_children(
    store: &TreeStore,
    mapping: &crate::tree::TreeNode,
) -> Result<bool, CoreError> {
    for pair in mapping.content.chunks_exact(2) {
        let value = node(store, pair[1])?;
        if value.kind == TreeNodeKind::Mapping && value.encode_separate() {
            return Ok(true);
        }
        if value.kind == TreeNodeKind::Sequence && all_maps(store, value)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn has_attributes(store: &TreeStore, mapping: &crate::tree::TreeNode) -> Result<bool, CoreError> {
    for pair in mapping.content.chunks_exact(2) {
        if is_attribute_value(store, pair[1], node(store, pair[1])?)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_attribute_value(
    store: &TreeStore,
    node_id: NodeId,
    value: &crate::tree::TreeNode,
) -> Result<bool, CoreError> {
    match value.kind {
        TreeNodeKind::Scalar if is_nil_scalar(store, node_id)? => Ok(false),
        TreeNodeKind::Scalar => Ok(true),
        TreeNodeKind::Sequence => Ok(value.content.is_empty() || !all_maps(store, value)?),
        TreeNodeKind::Mapping => Ok(!should_encode_as_separate_mapping(store, value)?),
        TreeNodeKind::Alias | TreeNodeKind::Unknown => {
            Err(CoreError::Parse(ParseError::UnsupportedTomlValue))
        }
    }
}

fn is_structural_value(
    store: &TreeStore,
    value: &crate::tree::TreeNode,
) -> Result<bool, CoreError> {
    match value.kind {
        TreeNodeKind::Sequence => Ok(!value.content.is_empty() && all_maps(store, value)?),
        TreeNodeKind::Mapping => should_encode_as_separate_mapping(store, value),
        TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => Ok(false),
    }
}

fn is_root_attribute_value(
    store: &TreeStore,
    node_id: NodeId,
    value: &crate::tree::TreeNode,
) -> Result<bool, CoreError> {
    if value.kind == TreeNodeKind::Mapping && !value.content.is_empty() {
        return Ok(false);
    }
    is_attribute_value(store, node_id, value)
}

fn is_root_structural_value(
    store: &TreeStore,
    node_id: NodeId,
    value: &crate::tree::TreeNode,
) -> Result<bool, CoreError> {
    if value.kind == TreeNodeKind::Mapping && !value.content.is_empty() {
        return Ok(true);
    }
    if is_attribute_value(store, node_id, value)? {
        return Ok(false);
    }
    is_structural_value(store, value)
}

fn all_maps(store: &TreeStore, node: &crate::tree::TreeNode) -> Result<bool, CoreError> {
    for child in &node.content {
        if node_kind(store, *child)? != TreeNodeKind::Mapping {
            return Ok(false);
        }
    }
    Ok(true)
}

fn node_kind(store: &TreeStore, node_id: NodeId) -> Result<TreeNodeKind, CoreError> {
    Ok(node(store, node_id)?.kind)
}

fn format_scalar(store: &TreeStore, node_id: NodeId) -> Result<String, CoreError> {
    let node = node(store, node_id)?;
    let raw = store.value_for(node_id)?;
    if node.tag_str() == TIMESTAMP_TAG {
        return Ok(raw.to_owned());
    }
    match node.get_value_rep_with(raw)? {
        ValueRep::Nil => Err(CoreError::Parse(ParseError::UnsupportedTomlValue)),
        ValueRep::Boolean(value) => Ok(value.to_string()),
        ValueRep::Int(value) => Ok(value.to_string()),
        ValueRep::Float(_) => Ok(raw.to_owned()),
        ValueRep::Str(value) => Ok(quote_toml_string(&value)),
    }
}

fn write_path(path: &[&str], out: &mut String) {
    for (index, key) in path.iter().enumerate() {
        if index > 0 {
            out.push('.');
        }
        write_key(key, out);
    }
}

fn write_key(key: &str, out: &mut String) {
    out.push_str(&format_key(key));
}

fn format_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        key.to_string()
    } else {
        quote_toml_string(key)
    }
}

fn quote_toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

pub fn encode_toml(store: &TreeStore, node: NodeId) -> Result<String, CoreError> {
    TomlEncoder::default().encode_to_string(store, node)
}

#[cfg(test)]
mod tests {
    use super::{TomlEncoder, encode_toml};
    use crate::errors::{CoreError, ParseError};
    use crate::formats::{Encode, FormatPreferences};
    use crate::language::SemType;
    use crate::tree::{CompactTag, NodeId, TreeNode, TreeNodeKind, TreeStore};

    fn scalar(sem_type: SemType, value: &str) -> TreeNode {
        TreeNode::scalar(sem_type, value)
    }

    fn map_node() -> TreeNode {
        TreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            tag: CompactTag::from_sem_type(SemType::Map),
            ..TreeNode::default()
        }
    }

    fn seq_node() -> TreeNode {
        TreeNode {
            kind: TreeNodeKind::Sequence,
            sem_type: Some(SemType::Seq),
            tag: CompactTag::from_sem_type(SemType::Seq),
            ..TreeNode::default()
        }
    }

    fn add_entry(store: &mut TreeStore, parent: NodeId, key: &str, value: TreeNode) -> NodeId {
        let (_, value_id) = store
            .add_key_value_child(parent, scalar(SemType::Str, key), value)
            .expect("map entry should be added");
        value_id
    }

    #[test]
    fn toml_encoder_writes_attributes_tables_and_array_tables() {
        let mut store = TreeStore::new();
        let root = store.add(map_node());
        add_entry(
            &mut store,
            root,
            "title",
            scalar(SemType::Str, "TOML Example"),
        );

        let owner = add_entry(&mut store, root, "owner", map_node());
        add_entry(&mut store, owner, "name", scalar(SemType::Str, "Tom"));

        let products = add_entry(&mut store, root, "products", seq_node());
        let hammer = store
            .add_child(products, map_node())
            .expect("array-table item should be added");
        add_entry(&mut store, hammer, "name", scalar(SemType::Str, "Hammer"));
        let nail = store
            .add_child(products, map_node())
            .expect("array-table item should be added");
        add_entry(&mut store, nail, "name", scalar(SemType::Str, "Nail"));

        let out = encode_toml(&store, root).expect("toml encoding should succeed");

        assert!(out.contains("title = \"TOML Example\"\n"));
        assert!(out.contains("[owner]\nname = \"Tom\"\n"));
        assert!(out.contains("[[products]]\nname = \"Hammer\"\n"));
        assert!(out.contains("[[products]]\nname = \"Nail\"\n"));
    }

    #[test]
    fn toml_encoder_omits_nested_empty_arrays_and_keeps_root_empty_arrays() {
        let mut store = TreeStore::new();
        let root = store.add(map_node());

        add_entry(&mut store, root, "root_empty", seq_node());

        let object = add_entry(&mut store, root, "object", map_node());
        add_entry(&mut store, object, "arr0", seq_node());
        add_entry(&mut store, object, "obj0", map_node());

        let out = encode_toml(&store, root).expect("toml encoding should succeed");

        assert!(out.contains("root_empty = []\n"));
        assert!(out.contains("[object]\nobj0 = {}\n"));
        assert!(!out.contains("arr0 = []\n"));
    }

    #[test]
    fn toml_encoder_inlines_non_structural_nested_maps() {
        let mut store = TreeStore::new();
        let root = store.add(map_node());
        let parent = add_entry(&mut store, root, "parent", map_node());
        let child = add_entry(&mut store, parent, "child", map_node());
        add_entry(&mut store, child, "name", scalar(SemType::Str, "Tom"));

        let out = encode_toml(&store, root).expect("toml encoding should succeed");

        assert!(out.contains("[parent]\nchild = { name = \"Tom\" }\n"));
        assert!(!out.contains("[parent.child]\n"));
    }

    #[test]
    fn toml_encoder_formats_inline_and_multiline_arrays_like_zig() {
        let mut store = TreeStore::new();
        let root = store.add(map_node());

        let short = add_entry(&mut store, root, "short", seq_node());
        store.add_child(short, scalar(SemType::Str, "a")).unwrap();
        store.add_child(short, scalar(SemType::Str, "b")).unwrap();
        store.add_child(short, scalar(SemType::Str, "c")).unwrap();

        let long = add_entry(&mut store, root, "long", seq_node());
        store
            .add_child(
                long,
                scalar(
                    SemType::Str,
                    "https%3A%2F%2Ftreease.com%2Fpreview%3Ffrom%3Dhover",
                ),
            )
            .unwrap();
        store
            .add_child(
                long,
                scalar(
                    SemType::Str,
                    "https://treease.com/path?redirect=https%3A%2F%2Ftreease.com%2Fdone",
                ),
            )
            .unwrap();

        let out = encode_toml(&store, root).expect("toml encoding should succeed");

        assert!(out.contains("short = [\"a\", \"b\", \"c\"]\n"));
        assert!(out.contains("long = [\n  \"https%3A%2F%2Ftreease.com%2Fpreview%3Ffrom%3Dhover\",\n  \"https://treease.com/path?redirect=https%3A%2F%2Ftreease.com%2Fdone\"\n]\n"));
    }

    #[test]
    fn toml_encoder_omits_trailing_newline_for_zero_indent_scalar() {
        let mut store = TreeStore::new();
        let root = store.add(scalar(SemType::Int, "42"));
        let encoder = TomlEncoder::new(FormatPreferences {
            indent: 0,
            ..FormatPreferences::default()
        });

        let out = encoder
            .encode_to_string(&store, root)
            .expect("scalar toml encoding should succeed");

        assert_eq!(out, "42");
    }

    #[test]
    fn toml_encoder_preserves_timestamp_scalars_without_quoting() {
        let mut store = TreeStore::new();
        let mut timestamp = scalar(SemType::Str, "1979-05-27T07:32:00Z");
        timestamp.tag = CompactTag::from_text("!!timestamp");
        let root = store.add(timestamp);

        let out = encode_toml(&store, root).expect("timestamp toml encoding should succeed");

        assert_eq!(out, "1979-05-27T07:32:00Z\n");
    }

    #[test]
    fn toml_encoder_rejects_nil_scalars() {
        let mut store = TreeStore::new();
        let root = store.add(TreeNode::scalar(SemType::Nil, "null"));

        let err = encode_toml(&store, root).expect_err("nil scalar should be rejected");
        assert!(matches!(
            err,
            CoreError::Parse(ParseError::UnsupportedTomlValue)
        ));
    }
}
