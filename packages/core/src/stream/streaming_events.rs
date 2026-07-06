use std::convert::Infallible;

use crate::language::SemType;

/// Trait for supplying a path string. Unlike a plain `fn() -> String`,
/// this trait can carry state (e.g. a captured allocator or context).
pub trait PathSupplier {
    fn get_path(&self) -> String;
}

/// A `PathSupplier` that wraps a closure carrying state.
impl<F: Fn() -> String> PathSupplier for F {
    fn get_path(&self) -> String {
        (self)()
    }
}

#[derive(Default)]
pub struct Meta {
    pub tag: String,
    pub sem_type: Option<SemType>,
    pub start_byte: u32,
    pub end_byte: u32,
    pub line: i32,
    pub column: i32,
    pub path: String,
    pub path_supplier: Option<Box<dyn PathSupplier>>,
    pub document: u32,
    pub filename: String,
    pub file_index: i32,
    pub anchor: String,
    pub head_comment: String,
    pub line_comment: String,
    pub foot_comment: String,
}

impl std::fmt::Debug for Meta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Meta")
            .field("tag", &self.tag)
            .field("sem_type", &self.sem_type)
            .field("start_byte", &self.start_byte)
            .field("end_byte", &self.end_byte)
            .field("line", &self.line)
            .field("column", &self.column)
            .field("path", &self.path)
            .field("document", &self.document)
            .field("filename", &self.filename)
            .field("file_index", &self.file_index)
            .field("anchor", &self.anchor)
            .field("head_comment", &self.head_comment)
            .field("line_comment", &self.line_comment)
            .field("foot_comment", &self.foot_comment)
            .finish()
    }
}

impl Clone for Meta {
    fn clone(&self) -> Self {
        Meta {
            tag: self.tag.clone(),
            sem_type: self.sem_type,
            start_byte: self.start_byte,
            end_byte: self.end_byte,
            line: self.line,
            column: self.column,
            path: self.path.clone(),
            path_supplier: None,
            document: self.document,
            filename: self.filename.clone(),
            file_index: self.file_index,
            anchor: self.anchor.clone(),
            head_comment: self.head_comment.clone(),
            line_comment: self.line_comment.clone(),
            foot_comment: self.foot_comment.clone(),
        }
    }
}

impl PartialEq for Meta {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag
            && self.sem_type == other.sem_type
            && self.start_byte == other.start_byte
            && self.end_byte == other.end_byte
            && self.line == other.line
            && self.column == other.column
            && self.path == other.path
            && self.document == other.document
            && self.filename == other.filename
            && self.file_index == other.file_index
            && self.anchor == other.anchor
            && self.head_comment == other.head_comment
            && self.line_comment == other.line_comment
            && self.foot_comment == other.foot_comment
    }
}

impl Eq for Meta {}

impl Meta {
    pub fn resolved_path(&self) -> String {
        if !self.path.is_empty() {
            return self.path.clone();
        }
        self.path_supplier
            .as_ref()
            .map(|s| s.get_path())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamingEvent {
    DocStart(Meta),
    DocEnd(Meta),
    MapStart(Meta),
    MapKey {
        value: String,
        meta: Meta,
    },
    MapEnd(Meta),
    SeqStart(Meta),
    SeqEnd(Meta),
    /// Scalar event. The semantic type is carried in `meta.sem_type`,
    Scalar {
        value: String,
        meta: Meta,
    },
    Alias {
        anchor: String,
        meta: Meta,
    },
    ParseError {
        message: String,
        meta: Meta,
    },
}

impl StreamingEvent {
    pub fn tag(&self) -> &'static str {
        match self {
            StreamingEvent::DocStart(_) => "doc_start",
            StreamingEvent::DocEnd(_) => "doc_end",
            StreamingEvent::MapStart(_) => "map_start",
            StreamingEvent::MapKey { .. } => "map_key",
            StreamingEvent::MapEnd(_) => "map_end",
            StreamingEvent::SeqStart(_) => "seq_start",
            StreamingEvent::SeqEnd(_) => "seq_end",
            StreamingEvent::Scalar { .. } => "scalar",
            StreamingEvent::Alias { .. } => "alias",
            StreamingEvent::ParseError { .. } => "parse_error",
        }
    }
}

pub trait EventSink {
    type Error;

    fn emit(&mut self, event: StreamingEvent) -> Result<(), Self::Error>;
}

impl EventSink for Vec<StreamingEvent> {
    type Error = Infallible;

    fn emit(&mut self, event: StreamingEvent) -> Result<(), Self::Error> {
        self.push(event);
        Ok(())
    }
}

pub struct FanOutSink<'a, E> {
    sinks: Vec<&'a mut dyn EventSink<Error = E>>,
}

impl<'a, E> FanOutSink<'a, E> {
    pub fn new(sinks: Vec<&'a mut dyn EventSink<Error = E>>) -> Self {
        Self { sinks }
    }
}

impl<E> EventSink for FanOutSink<'_, E> {
    type Error = E;

    fn emit(&mut self, event: StreamingEvent) -> Result<(), Self::Error> {
        for sink in &mut self.sinks {
            sink.emit(event.clone())?;
        }
        Ok(())
    }
}
