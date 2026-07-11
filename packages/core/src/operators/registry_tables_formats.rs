use crate::{
    core::{
        self, CoreError as CoreCodecError, FormatError as CoreFormatError, FormatLanguage,
        ParseError as CoreParseError, TreeNodeKind as CoreTreeNodeKind,
    },
    formats::{
        self, CsvEncoder, CsvObjectDecoder, Decode as ValueDecoder, Encode as ValueEncoder,
        JavascriptEncoder, JavascriptObjectDecoder, JsonDecoder, JsonEncoder, PythonEncoder,
        PythonObjectDecoder, TomlDecoder, TomlEncoder, YamlDecoder, YamlEncoder,
        configured_language_preferences,
    },
    operators::*,
};

#[derive(Clone, Copy)]
pub struct FormatEntry {
    pub name: &'static str,
    pub encoder_factory: Option<EncoderFactory>,
    pub decoder_factory: Option<DecoderFactory>,
}

pub type EncoderFactory = fn(ctx: &Context, indent: i32) -> Result<Box<dyn Encoder>, CoreError>;
pub type DecoderFactory = fn(ctx: &Context) -> Result<Box<dyn Decoder>, CoreError>;

pub trait Encoder {
    fn encode_to_string(&self, candidate: &TreeNode) -> Result<String, CoreError>;

    fn can_handle_aliases(&self) -> bool {
        true
    }

    fn deinit(&mut self) {}
}

pub trait Decoder {
    fn decode_string(&self, input: &str) -> Result<Box<TreeNode>, CoreError>;

    fn deinit(&mut self) {}
}

pub struct FormatFlags {
    pub json: bool,
    pub yaml: bool,
    pub toml: bool,
    pub python: bool,
    pub javascript: bool,
    pub csv: bool,
}

impl Default for FormatFlags {
    fn default() -> Self {
        Self {
            json: true,
            yaml: true,
            toml: true,
            python: true,
            javascript: true,
            csv: true,
        }
    }
}

struct FormatDef {
    name: &'static str,
    enc: Option<EncoderFactory>,
    dec: Option<DecoderFactory>,
}

struct EncodeAdapter<E> {
    inner: E,
    can_handle_aliases: bool,
}

impl<E> Encoder for EncodeAdapter<E>
where
    E: ValueEncoder,
{
    fn encode_to_string(&self, candidate: &TreeNode) -> Result<String, CoreError> {
        let (store, root) = compat_tree_to_core(candidate)?;
        let root = if self.can_handle_aliases {
            root
        } else {
            formats::formats_helpers::resolve_alias_for_encode(&store, root)
                .map_err(map_core_error)?
                .unwrap_or(root)
        };
        self.inner
            .encode_to_string(&store, root)
            .map_err(map_core_error)
    }

    fn can_handle_aliases(&self) -> bool {
        self.can_handle_aliases
    }
}

struct DecodeAdapter<D> {
    inner: D,
}

impl<D> Decoder for DecodeAdapter<D>
where
    D: ValueDecoder,
{
    fn decode_string(&self, input: &str) -> Result<Box<TreeNode>, CoreError> {
        let decoded = self.inner.decode_str(input).map_err(map_core_error)?;
        Ok(Box::new(core_tree_to_compat(&decoded.store, decoded.root)?))
    }
}

fn map_core_error(err: CoreCodecError) -> CoreError {
    match err {
        CoreCodecError::Format(CoreFormatError::UnknownFormat) => {
            CoreError::Format(FormatError::UnknownFormat)
        }
        CoreCodecError::Format(_) => CoreError::Format(FormatError::UnknownFormat),
        CoreCodecError::Parse(CoreParseError::InvalidJson) => {
            CoreError::Parse(ParseError::InvalidJson)
        }
        CoreCodecError::Parse(CoreParseError::InvalidYaml) => {
            CoreError::Parse(ParseError::InvalidYaml)
        }
        CoreCodecError::Parse(CoreParseError::InvalidPython) => {
            CoreError::Parse(ParseError::InvalidPython)
        }
        CoreCodecError::Parse(CoreParseError::InvalidJavaScript) => {
            CoreError::Parse(ParseError::InvalidJavaScript)
        }
        CoreCodecError::Parse(_) => CoreError::Parse(ParseError::InvalidSyntax),
        CoreCodecError::Eval(core::EvalError::MissingTreeNode) => {
            CoreError::Eval(EvalError::MissingTreeNode)
        }
        CoreCodecError::Eval(_) => CoreError::Eval(EvalError::UnsupportedFlat),
        CoreCodecError::System(_) | CoreCodecError::Io(_) => {
            CoreError::Format(FormatError::UnknownFormat)
        }
        CoreCodecError::ParseMessage { .. } => CoreError::Parse(ParseError::InvalidSyntax),
        CoreCodecError::OperatorMessage { .. } => CoreError::Eval(EvalError::UnsupportedFlat),
        CoreCodecError::WasmProtocol { .. } => CoreError::Format(FormatError::UnknownFormat),
        CoreCodecError::CapabilityMissing { .. } => CoreError::Format(FormatError::UnknownFormat),
        CoreCodecError::OutOfMemory => CoreError::OutOfMemory,
    }
}

fn to_core_kind(kind: NodeKind) -> CoreTreeNodeKind {
    match kind {
        NodeKind::Scalar => CoreTreeNodeKind::Scalar,
        NodeKind::Mapping => CoreTreeNodeKind::Mapping,
        NodeKind::Sequence => CoreTreeNodeKind::Sequence,
        NodeKind::Alias => CoreTreeNodeKind::Alias,
        NodeKind::Unknown => CoreTreeNodeKind::Unknown,
    }
}

fn from_core_kind(kind: CoreTreeNodeKind) -> NodeKind {
    match kind {
        CoreTreeNodeKind::Scalar | CoreTreeNodeKind::Unknown => NodeKind::Scalar,
        CoreTreeNodeKind::Mapping => NodeKind::Mapping,
        CoreTreeNodeKind::Sequence => NodeKind::Sequence,
        CoreTreeNodeKind::Alias => NodeKind::Alias,
    }
}

fn to_core_sem_type(sem_type: Option<SemType>) -> Option<core::SemType> {
    sem_type.and_then(|value| match value {
        SemType::Nil => Some(core::SemType::Nil),
        SemType::Str => Some(core::SemType::Str),
        SemType::Int => Some(core::SemType::Int),
        SemType::Float => Some(core::SemType::Float),
        SemType::Boolean => Some(core::SemType::Boolean),
        SemType::Map => Some(core::SemType::Map),
        SemType::Seq => Some(core::SemType::Seq),
    })
}

fn from_core_sem_type(sem_type: Option<core::SemType>) -> Option<SemType> {
    sem_type.map(|value| match value {
        core::SemType::Nil => SemType::Nil,
        core::SemType::Str => SemType::Str,
        core::SemType::Int => SemType::Int,
        core::SemType::Float => SemType::Float,
        core::SemType::Boolean => SemType::Boolean,
        core::SemType::Map => SemType::Map,
        core::SemType::Seq => SemType::Seq,
    })
}

fn compat_tree_to_core(root: &TreeNode) -> Result<(core::TreeStore, core::NodeId), CoreError> {
    let mut store = core::TreeStore::new();
    let root = append_compat_node(&mut store, root, None)?;
    Ok((store, root))
}

fn append_compat_node(
    store: &mut core::TreeStore,
    node: &TreeNode,
    parent: Option<core::NodeId>,
) -> Result<core::NodeId, CoreError> {
    let mut out = core::TreeNode {
        kind: to_core_kind(node.kind),
        sem_type: to_core_sem_type(node.sem_type),
        tag: core::CompactTag::from_text(node.tag.clone()),
        value: node.value.clone().into(),
        start_byte: node.start_byte,
        end_byte: node.end_byte,
        parent,
        document: node.document,
        line: node.line,
        column: node.column,
        is_map_key: node.is_map_key,
        ..core::TreeNode::default()
    };
    out.set_alias(node.alias.map(|id| core::NodeId(id.0 as u32)));
    out.set_key(node.key.map(|id| core::NodeId(id.0 as u32)));
    out.set_sequence_index(node.sequence_index.map(|index| index as u32));
    out.set_sequence_closed(node.sequence_closed);
    out.set_encode_separate(node.encode_separate);
    out.set_evaluate_together(node.evaluate_together);
    let id = store.add(out);
    store.set_document_meta(node.document, node.filename.clone(), node.file_index);
    let _ = store.set_anchor(id, node.anchor.clone());
    let _ = store.set_comments(
        id,
        node.head_comment.clone(),
        node.line_comment.clone(),
        node.foot_comment.clone(),
    );
    let _ = store.set_leading_content(id, node.leading_content.clone());

    let child_ids = node
        .content
        .iter()
        .map(|child| append_compat_node(store, child, Some(id)))
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(current) = store.get_mut(id) {
        current.content = child_ids.clone();
    }

    let current_kind = store.get(id).map(|current| current.kind);
    match current_kind {
        Some(CoreTreeNodeKind::Sequence) => {
            for (index, child_id) in child_ids.iter().enumerate() {
                if let Some(child) = store.get_mut(*child_id) {
                    child.set_sequence_index(Some(index as u32));
                }
            }
        }
        Some(CoreTreeNodeKind::Mapping) => {
            for pair in child_ids.chunks_exact(2) {
                if let Some(key) = store.get_mut(pair[0]) {
                    key.is_map_key = true;
                }
                if let Some(value) = store.get_mut(pair[1]) {
                    value.set_key(Some(pair[0]));
                    value.set_sequence_index(None);
                }
            }
        }
        _ => {}
    }

    Ok(id)
}

pub(crate) fn core_tree_to_compat(
    store: &core::TreeStore,
    root: core::NodeId,
) -> Result<TreeNode, CoreError> {
    let source = store
        .get(root)
        .ok_or(CoreError::Eval(EvalError::MissingTreeNode))?;
    let mut out = TreeNode {
        kind: from_core_kind(source.kind),
        sequence_closed: source.sequence_closed(),
        sem_type: from_core_sem_type(source.sem_type),
        tag: source.tag.to_string_value(),
        value: store.value_string_for(root).unwrap_or_default(),
        start_byte: source.start_byte,
        end_byte: source.end_byte,
        anchor: store.anchor_for(root).unwrap_or_default().to_owned(),
        alias: source.alias().map(|id| NodeId(id.index())),
        head_comment: store.head_comment_for(root).unwrap_or_default().to_owned(),
        line_comment: store.line_comment_for(root).unwrap_or_default().to_owned(),
        foot_comment: store.foot_comment_for(root).unwrap_or_default().to_owned(),
        parent: source.parent.map(|id| NodeId(id.index())),
        key: source.key().map(|id| NodeId(id.index())),
        sequence_index: source.sequence_index().map(|index| index as i64),
        leading_content: store
            .leading_content_for(root)
            .unwrap_or_default()
            .to_owned(),
        document: source.document,
        filename: store.filename_for(root).unwrap_or_default().to_owned(),
        line: source.line,
        column: source.column,
        file_index: store.file_index_for(root).unwrap_or_default(),
        is_map_key: source.is_map_key,
        encode_separate: source.encode_separate(),
        evaluate_together: source.evaluate_together(),
        ..TreeNode::default()
    };

    out.content = source
        .content
        .iter()
        .map(|child| core_tree_to_compat(store, *child))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(out)
}

fn prefs_for(
    language: FormatLanguage,
    indent: Option<i32>,
    unwrap_scalar: bool,
) -> formats::FormatPreferences {
    let mut prefs = configured_language_preferences().effective(language);
    if let Some(indent) = indent {
        prefs.indent = indent;
    }
    prefs.unwrap_scalar = unwrap_scalar;
    prefs
}

fn json_encoder(_ctx: &Context, indent: i32) -> Result<Box<dyn Encoder>, CoreError> {
    Ok(Box::new(EncodeAdapter {
        inner: JsonEncoder::new(prefs_for(FormatLanguage::Json, Some(indent), false)),
        can_handle_aliases: false,
    }))
}

fn json_decoder(_ctx: &Context) -> Result<Box<dyn Decoder>, CoreError> {
    Ok(Box::new(DecodeAdapter { inner: JsonDecoder }))
}

fn yaml_encoder(_ctx: &Context, indent: i32) -> Result<Box<dyn Encoder>, CoreError> {
    Ok(Box::new(EncodeAdapter {
        inner: YamlEncoder::new(prefs_for(FormatLanguage::Yaml, Some(indent.max(2)), false)),
        can_handle_aliases: true,
    }))
}

fn yaml_decoder(_ctx: &Context) -> Result<Box<dyn Decoder>, CoreError> {
    Ok(Box::new(DecodeAdapter { inner: YamlDecoder }))
}

fn toml_encoder(_ctx: &Context, _indent: i32) -> Result<Box<dyn Encoder>, CoreError> {
    Ok(Box::new(EncodeAdapter {
        inner: TomlEncoder::new(prefs_for(FormatLanguage::Toml, None, true)),
        can_handle_aliases: true,
    }))
}

fn toml_decoder(_ctx: &Context) -> Result<Box<dyn Decoder>, CoreError> {
    Ok(Box::new(DecodeAdapter { inner: TomlDecoder }))
}

fn csv_encoder(_ctx: &Context, _indent: i32) -> Result<Box<dyn Encoder>, CoreError> {
    Ok(Box::new(EncodeAdapter {
        inner: CsvEncoder::new(prefs_for(FormatLanguage::Csv, None, true)),
        can_handle_aliases: false,
    }))
}

fn csv_decoder(_ctx: &Context) -> Result<Box<dyn Decoder>, CoreError> {
    Ok(Box::new(DecodeAdapter {
        inner: CsvObjectDecoder::new(prefs_for(FormatLanguage::Csv, None, true)),
    }))
}

fn python_encoder(_ctx: &Context, indent: i32) -> Result<Box<dyn Encoder>, CoreError> {
    Ok(Box::new(EncodeAdapter {
        inner: PythonEncoder::new(prefs_for(FormatLanguage::Python, Some(indent), false)),
        can_handle_aliases: false,
    }))
}

fn python_decoder(_ctx: &Context) -> Result<Box<dyn Decoder>, CoreError> {
    Ok(Box::new(DecodeAdapter {
        inner: PythonObjectDecoder,
    }))
}

fn javascript_encoder(_ctx: &Context, indent: i32) -> Result<Box<dyn Encoder>, CoreError> {
    Ok(Box::new(EncodeAdapter {
        inner: JavascriptEncoder::new(prefs_for(FormatLanguage::Javascript, Some(indent), false)),
        can_handle_aliases: false,
    }))
}

fn javascript_decoder(_ctx: &Context) -> Result<Box<dyn Decoder>, CoreError> {
    Ok(Box::new(DecodeAdapter {
        inner: JavascriptObjectDecoder,
    }))
}

fn json_entries() -> Vec<FormatDef> {
    vec![FormatDef {
        name: "json",
        enc: Some(json_encoder),
        dec: Some(json_decoder),
    }]
}

fn yaml_entries() -> Vec<FormatDef> {
    vec![FormatDef {
        name: "yaml",
        enc: Some(yaml_encoder),
        dec: Some(yaml_decoder),
    }]
}

fn toml_entries() -> Vec<FormatDef> {
    vec![FormatDef {
        name: "toml",
        enc: Some(toml_encoder),
        dec: Some(toml_decoder),
    }]
}

fn csv_entries() -> Vec<FormatDef> {
    vec![FormatDef {
        name: "csv",
        enc: Some(csv_encoder),
        dec: Some(csv_decoder),
    }]
}

fn python_entries() -> Vec<FormatDef> {
    vec![FormatDef {
        name: "python",
        enc: Some(python_encoder),
        dec: Some(python_decoder),
    }]
}

fn javascript_entries() -> Vec<FormatDef> {
    vec![FormatDef {
        name: "javascript",
        enc: Some(javascript_encoder),
        dec: Some(javascript_decoder),
    }]
}

/// append_formats: collect all enabled format entries into the given Vec.
pub fn append_formats(entries: &mut Vec<FormatEntry>, flags: &FormatFlags) {
    let defs: Vec<(&str, Vec<FormatDef>)> = vec![
        ("json", json_entries()),
        ("yaml", yaml_entries()),
        ("toml", toml_entries()),
        ("csv", csv_entries()),
        ("python", python_entries()),
        ("javascript", javascript_entries()),
    ];

    let enabled: Vec<bool> = vec![
        flags.json,
        flags.yaml,
        flags.toml,
        flags.csv,
        flags.python,
        flags.javascript,
    ];

    for (enabled_flag, (_, defs)) in enabled.iter().zip(defs.iter()) {
        if *enabled_flag {
            for def in defs {
                entries.push(FormatEntry {
                    name: def.name,
                    encoder_factory: def.enc,
                    decoder_factory: def.dec,
                });
            }
        }
    }
}
