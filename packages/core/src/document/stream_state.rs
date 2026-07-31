use crate::analysis::LineIndex;
use crate::graph::streaming_graph_projector::StreamingGraphProjector;
use crate::language::{stream_kind_for_language, StreamKind};
use crate::stream::streaming_events::{Meta, StreamingEvent};
use crate::stream::streaming_json::streaming_parse::clamp_offset_to_u32;
use crate::stream::streaming_json::SourceRewrite;
use crate::stream::tree_builder::Builder;

#[derive(Debug, Default)]
pub(crate) struct StreamingSourceDoc {
    raw: Vec<u8>,
    raw_base_offset: usize,
    raw_cursor: usize,
    source: String,
    line_starts: Vec<usize>,
    rewrites: Vec<AppliedRewrite>,
}

#[derive(Debug, Clone, Copy)]
struct AppliedRewrite {
    raw_start: usize,
    raw_end: usize,
    source_start: usize,
    replacement_len: usize,
    cumulative_delta_after: i64,
}

#[derive(Debug, Default)]
pub(crate) struct SourceDocUpdate {
    pub appended_line_count: u32,
    pub processed_bytes: u32,
}

#[derive(Debug)]
pub(crate) enum CommitEventsError<E> {
    Source(std::str::Utf8Error),
    Callback(E),
}

impl StreamingSourceDoc {
    pub(crate) fn new() -> Self {
        Self {
            line_starts: vec![0],
            ..Self::default()
        }
    }

    pub(crate) fn push_input(&mut self, bytes: &[u8]) {
        self.raw.extend_from_slice(bytes);
    }

    #[allow(dead_code)]
    pub(crate) fn commit_events(
        &mut self,
        events: Vec<StreamingEvent>,
        rewrites: Vec<SourceRewrite>,
    ) -> Result<(Vec<StreamingEvent>, SourceDocUpdate), std::str::Utf8Error> {
        let mut rebased_events = Vec::new();
        let source_update = match self.commit_events_with(events, rewrites, |event| {
            rebased_events.push(event);
            Ok::<(), std::convert::Infallible>(())
        }) {
            Ok(update) => update,
            Err(CommitEventsError::Source(error)) => return Err(error),
            Err(CommitEventsError::Callback(error)) => match error {},
        };
        Ok((rebased_events, source_update))
    }

    pub(crate) fn commit_events_with<E>(
        &mut self,
        events: Vec<StreamingEvent>,
        rewrites: Vec<SourceRewrite>,
        mut on_event: impl FnMut(StreamingEvent) -> Result<(), E>,
    ) -> Result<SourceDocUpdate, CommitEventsError<E>> {
        let line_count_before = self.line_starts.len();
        for rewrite in rewrites {
            self.apply_rewrite(rewrite)
                .map_err(CommitEventsError::Source)?;
        }
        let max_event_end = events
            .iter()
            .filter_map(event_commit_end)
            .max()
            .unwrap_or(self.raw_cursor as u32) as usize;
        self.commit_raw_until(max_event_end)
            .map_err(CommitEventsError::Source)?;
        for event in events {
            on_event(self.rebase_event(event)).map_err(CommitEventsError::Callback)?;
        }
        Ok(SourceDocUpdate {
            appended_line_count: self.line_starts.len().saturating_sub(line_count_before) as u32,
            processed_bytes: self.processed_bytes(),
        })
    }

    pub(crate) fn finish(&mut self) -> Result<SourceDocUpdate, std::str::Utf8Error> {
        let line_count_before = self.line_starts.len();
        self.commit_raw_until(self.raw_base_offset + self.raw.len())?;
        Ok(SourceDocUpdate {
            appended_line_count: self.line_starts.len().saturating_sub(line_count_before) as u32,
            processed_bytes: self.processed_bytes(),
        })
    }

    pub(crate) fn source_text(&self) -> &str {
        &self.source
    }

    pub(crate) fn line_index(&self) -> LineIndex {
        LineIndex::from_line_starts_and_len(self.line_starts.clone(), self.source.len())
    }
    pub(crate) fn processed_bytes(&self) -> u32 {
        (self.raw_base_offset + self.raw.len()).min(u32::MAX as usize) as u32
    }
    fn apply_rewrite(&mut self, rewrite: SourceRewrite) -> Result<(), std::str::Utf8Error> {
        let raw_start = rewrite.start_byte as usize;
        let raw_end = rewrite.old_end_byte as usize;
        if raw_end <= self.raw_cursor {
            return Ok(());
        }
        self.commit_raw_until(raw_start)?;
        let source_start = self.source.len();
        self.append_source_text(&rewrite.replacement);
        self.raw_cursor = raw_end;
        let cumulative_delta_after = self.source.len() as i64 - self.raw_cursor as i64;
        self.rewrites.push(AppliedRewrite {
            raw_start,
            raw_end,
            source_start,
            replacement_len: rewrite.replacement.len(),
            cumulative_delta_after,
        });
        self.compact_committed_raw();
        Ok(())
    }

    fn commit_raw_until(&mut self, end: usize) -> Result<(), std::str::Utf8Error> {
        let end = end.min(self.raw_base_offset + self.raw.len());
        if end <= self.raw_cursor {
            return Ok(());
        }
        let start_index = self.raw_cursor.saturating_sub(self.raw_base_offset);
        let end_index = end.saturating_sub(self.raw_base_offset);
        let text = std::str::from_utf8(&self.raw[start_index..end_index])?;
        append_source_text_parts(&mut self.source, &mut self.line_starts, text);
        self.raw_cursor = end;
        self.compact_committed_raw();
        Ok(())
    }

    fn compact_committed_raw(&mut self) {
        let consumed = self.raw_cursor.saturating_sub(self.raw_base_offset);
        if consumed == 0 {
            return;
        }
        self.raw.drain(..consumed.min(self.raw.len()));
        self.raw_base_offset = self.raw_cursor;
    }

    fn append_source_text(&mut self, text: &str) {
        append_source_text_parts(&mut self.source, &mut self.line_starts, text);
    }

    fn rebase_event(&self, event: StreamingEvent) -> StreamingEvent {
        match event {
            StreamingEvent::DocStart(meta) => StreamingEvent::DocStart(self.rebase_meta(meta)),
            StreamingEvent::DocEnd(meta) => StreamingEvent::DocEnd(self.rebase_meta(meta)),
            StreamingEvent::MapStart(meta) => StreamingEvent::MapStart(self.rebase_meta(meta)),
            StreamingEvent::MapKey { value, meta } => StreamingEvent::MapKey {
                value,
                meta: self.rebase_meta(meta),
            },
            StreamingEvent::MapEnd(meta) => StreamingEvent::MapEnd(self.rebase_meta(meta)),
            StreamingEvent::SeqStart(meta) => StreamingEvent::SeqStart(self.rebase_meta(meta)),
            StreamingEvent::SeqEnd(meta) => StreamingEvent::SeqEnd(self.rebase_meta(meta)),
            StreamingEvent::Scalar { value, meta } => StreamingEvent::Scalar {
                value,
                meta: self.rebase_meta(meta),
            },
            StreamingEvent::Alias { anchor, meta } => StreamingEvent::Alias {
                anchor,
                meta: self.rebase_meta(meta),
            },
            StreamingEvent::ParseError { message, meta } => StreamingEvent::ParseError {
                message,
                meta: self.rebase_meta(meta),
            },
        }
    }

    fn rebase_meta(&self, mut meta: Meta) -> Meta {
        meta.start_byte = self.map_offset(meta.start_byte);
        meta.end_byte = self.map_offset(meta.end_byte);
        let line_column = self.offset_to_line_column(meta.start_byte as usize);
        meta.line = line_column.line as i32 + 1;
        meta.column = line_column.column as i32 + 1;
        meta
    }

    fn map_offset(&self, offset: u32) -> u32 {
        let raw = offset as usize;
        let index = self
            .rewrites
            .partition_point(|rewrite| rewrite.raw_start <= raw);
        if index == 0 {
            return offset;
        }
        let rewrite = &self.rewrites[index - 1];
        if raw <= rewrite.raw_end {
            let inner = raw.saturating_sub(rewrite.raw_start);
            let clamped = inner.min(rewrite.replacement_len);
            return (rewrite.source_start + clamped).min(u32::MAX as usize) as u32;
        }
        let mapped = raw as i64 + rewrite.cumulative_delta_after;
        clamp_offset_to_u32(mapped)
    }
    fn offset_to_line_column(&self, offset: usize) -> crate::analysis::LineColumn {
        let clamped = offset.min(self.source.len());
        let line_index = match self.line_starts.binary_search(&clamped) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0);
        crate::analysis::LineColumn {
            line: line_index as u32,
            column: (clamped - line_start) as u32,
        }
    }
}

fn append_source_text_parts(source: &mut String, line_starts: &mut Vec<usize>, text: &str) {
    let base = source.len();
    for (index, byte) in text.as_bytes().iter().enumerate() {
        if *byte == b'\n' {
            line_starts.push(base + index + 1);
        }
    }
    source.push_str(text);
}

fn event_commit_end(event: &StreamingEvent) -> Option<u32> {
    Some(match event {
        StreamingEvent::DocStart(meta)
        | StreamingEvent::DocEnd(meta)
        | StreamingEvent::MapStart(meta)
        | StreamingEvent::MapEnd(meta)
        | StreamingEvent::SeqStart(meta)
        | StreamingEvent::SeqEnd(meta) => meta.end_byte,
        StreamingEvent::MapKey { meta, .. }
        | StreamingEvent::Scalar { meta, .. }
        | StreamingEvent::Alias { meta, .. }
        | StreamingEvent::ParseError { meta, .. } => meta.end_byte,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_event(start: u32, end: u32) -> StreamingEvent {
        StreamingEvent::Scalar {
            value: String::new(),
            meta: Meta {
                start_byte: start,
                end_byte: end,
                ..Meta::default()
            },
        }
    }

    #[test]
    fn source_doc_keeps_only_pending_raw_bytes_after_commit() {
        let mut source = StreamingSourceDoc::new();
        source.push_input(b"abc");

        let _ = source
            .commit_events(vec![scalar_event(0, 2)], Vec::new())
            .expect("commit should succeed");

        assert_eq!(source.source_text(), "ab");
        assert_eq!(
            source.raw.len(),
            1,
            "committed raw bytes should not be retained alongside raw JobEntry and canonical source"
        );
    }

    #[test]
    fn source_doc_keeps_rewrite_offsets_correct_after_raw_compaction() {
        let mut source = StreamingSourceDoc::new();
        source.push_input(br#""{}" 1"#);

        let (events, _) = source
            .commit_events(
                vec![scalar_event(0, 4)],
                vec![SourceRewrite {
                    start_byte: 0,
                    old_end_byte: 4,
                    replacement: "{}".to_owned(),
                }],
            )
            .expect("rewrite should succeed");
        let first = match &events[0] {
            StreamingEvent::Scalar { meta, .. } => meta,
            _ => panic!("expected scalar"),
        };
        assert_eq!((first.start_byte, first.end_byte), (0, 2));
        assert_eq!(source.source_text(), "{}");
        assert_eq!(source.raw.len(), 2);

        source.push_input(b"2");
        let (events, _) = source
            .commit_events(vec![scalar_event(5, 7)], Vec::new())
            .expect("second commit should succeed");
        let second = match &events[0] {
            StreamingEvent::Scalar { meta, .. } => meta,
            _ => panic!("expected scalar"),
        };
        assert_eq!((second.start_byte, second.end_byte), (3, 5));
        assert_eq!(source.source_text(), "{} 12");
    }
}

pub(crate) fn supports_streaming_language(language: &str) -> bool {
    StreamState::supports_language(language)
}

/// Streaming parse state including persistent graph projector and
/// incrementally-accumulated source analysis (line offsets, semantic tokens).
#[derive(Debug)]
pub(crate) enum StreamState {
    Json {
        decoder: crate::stream::streaming_json::StreamDecoder,
        builder: Builder,
        first_chunk: bool,
        projector: Option<Box<StreamingGraphProjector>>,
        source_doc: StreamingSourceDoc,
        token_spans: Vec<crate::tree::TokenSpan>,
    },
}

impl StreamState {
    pub(crate) fn supports_language(language: &str) -> bool {
        matches!(stream_kind_for_language(language), StreamKind::Json)
    }

    pub(crate) fn for_language(
        language: &str,
        settings: crate::document::protocol::DocumentJobSettings,
        graph_output: bool,
    ) -> Option<Self> {
        let mut builder = Builder::new();
        builder.enable_patches();
        match stream_kind_for_language(language) {
            StreamKind::Json => {
                let decoder =
                    crate::stream::streaming_json::StreamDecoder::new(settings.parser.enable_nest);
                Some(Self::Json {
                    decoder,
                    builder,
                    first_chunk: true,
                    projector: graph_output
                        .then(|| Box::new(StreamingGraphProjector::new(language))),
                    source_doc: StreamingSourceDoc::new(),
                    token_spans: Vec::new(),
                })
            }
            StreamKind::NonStreaming => None,
        }
    }

    pub(crate) fn is_first_chunk(&self) -> bool {
        matches!(
            self,
            Self::Json {
                first_chunk: true,
                ..
            }
        )
    }

    pub(crate) fn source_len(&self) -> u32 {
        match self {
            Self::Json { source_doc, .. } => {
                (source_doc.source_text().len().min(u32::MAX as usize)) as u32
            }
        }
    }
    pub(crate) fn take_token_spans(&mut self) -> Vec<crate::tree::TokenSpan> {
        match self {
            Self::Json { token_spans, .. } => std::mem::take(token_spans),
        }
    }

    /// Returns `true` if the parser expanded inline content during finalisation
    /// (e.g. nested-JSON expansion), which rewrote the source text.
    /// The caller uses this to decide whether to propagate the rewritten
    /// source back to the viewer.
    pub(crate) fn has_expanded_source(&self) -> bool {
        match self {
            Self::Json { decoder, .. } => decoder.nested_json_expanded(),
        }
    }
}
