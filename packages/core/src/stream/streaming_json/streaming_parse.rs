use std::sync::{Mutex, OnceLock};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::{
    core::{CoreError, ParseError, SemType},
    formats::DecodedDocument,
    stream::{
        streaming_events::{EventSink, Meta, StreamingEvent},
        tree_builder::Builder,
    },
};

use super::{
    diagnostics::{ErrorSpan, TokenSpan},
    parse_tree::{self, DecodeWithTokenSpansResult},
    parser::JsonStreamError,
    scanner::{ProfileHooks, Scanner, Span, Token, TokenCache, TokenTag},
};

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

const TOKEN_TYPE_KEY: u32 = 1;
const TOKEN_TYPE_STRING: u32 = 3;
const TOKEN_TYPE_INT: u32 = 4;
const TOKEN_TYPE_BOOLEAN: u32 = 6;
const TOKEN_TYPE_NULL: u32 = 7;
const TOKEN_TYPE_PUNCTUATION: u32 = 8;
const TOKEN_TYPE_COMMENT: u32 = 9;

// ---------------------------------------------------------------------------
// DecodeProfile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRewrite {
    pub start_byte: u32,
    pub old_end_byte: u32,
    pub replacement: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AppliedSourceRewrite {
    raw_start: usize,
    raw_end: usize,
    source_start: usize,
    replacement_len: usize,
    cumulative_delta_after: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct NestedMaterialization {
    source: String,
    events: Vec<StreamingEvent>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecodeProfile {
    pub input_bytes: usize,
    pub document_count: u32,
    pub token_count: u32,
    pub event_count: u32,
    pub error_count: u32,
    /// Total wall-clock time in microseconds.
    pub total_us: u32,
    /// Time spent inside `Scanner::next_token` in microseconds.
    pub scanner_next_token_us: u32,
    /// Time spent emitting events (sink overhead) in microseconds.
    pub sink_emit_us: u32,
    /// Time spent finishing string tokens in microseconds.
    pub finish_string_token_us: u32,
    /// Time spent finishing number tokens in microseconds.
    pub finish_number_token_us: u32,
    /// Time spent normalising number values in microseconds.
    pub normalized_number_us: u32,
}

static LAST_DECODE_PROFILE: OnceLock<Mutex<DecodeProfile>> = OnceLock::new();

pub(crate) fn profile_cell() -> &'static Mutex<DecodeProfile> {
    LAST_DECODE_PROFILE.get_or_init(|| Mutex::new(DecodeProfile::default()))
}

pub fn reset_last_decode_profile() {
    if let Ok(mut profile) = profile_cell().lock() {
        *profile = DecodeProfile::default();
    }
}

pub fn get_last_decode_profile() -> DecodeProfile {
    profile_cell()
        .lock()
        .map(|profile| *profile)
        .unwrap_or_default()
}

pub(super) fn record_decode_profile(
    source: &str,
    events: usize,
    token_spans: usize,
    error_spans: usize,
) {
    if let Ok(mut profile) = profile_cell().lock() {
        profile.input_bytes = source.len();
        profile.document_count = if source.trim().is_empty() { 0 } else { 1 };
        profile.token_count = token_spans as u32;
        profile.event_count = events as u32;
        profile.error_count = error_spans as u32;
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// Base instant for the monotonic nanosecond clock.
#[cfg(not(target_arch = "wasm32"))]
static BASE_INSTANT: OnceLock<Instant> = OnceLock::new();

/// Return a monotonic timestamp in nanoseconds.
///
/// All calls share a single base [`Instant`] so that deltas between any
/// two calls are meaningful.  The absolute value is arbitrary; only
/// differences matter.
pub fn monotonic_now_ns() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        0
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let base = BASE_INSTANT.get_or_init(Instant::now);
        base.elapsed().as_nanos() as u64
    }
}

/// Convert nanoseconds to microseconds, saturating at `u32::MAX`.
pub fn duration_us(ns: u64) -> u32 {
    (ns / 1_000).min(u32::MAX as u64) as u32
}

/// Add a nanosecond delta to a `u32` microsecond accumulator, saturating.
fn add_duration_us(target: &mut u32, ns: u64) {
    let delta = duration_us(ns);
    let sum = *target as u64 + delta as u64;
    *target = sum.min(u32::MAX as u64) as u32;
}

/// Increment a `u32` counter, saturating at `u32::MAX`.
fn increment_counter(target: &mut u32) {
    if *target != u32::MAX {
        *target += 1;
    }
}

pub fn add_scanner_next_token_ns(ns: u64) {
    if let Ok(mut profile) = profile_cell().lock() {
        add_duration_us(&mut profile.scanner_next_token_us, ns);
    }
}

pub fn add_finish_string_token_ns(ns: u64) {
    if let Ok(mut profile) = profile_cell().lock() {
        add_duration_us(&mut profile.finish_string_token_us, ns);
    }
}

pub fn add_finish_number_token_ns(ns: u64) {
    if let Ok(mut profile) = profile_cell().lock() {
        add_duration_us(&mut profile.finish_number_token_us, ns);
    }
}

pub fn add_sink_emit_ns(ns: u64) {
    if let Ok(mut profile) = profile_cell().lock() {
        add_duration_us(&mut profile.sink_emit_us, ns);
    }
}

pub fn add_normalized_number_ns(ns: u64) {
    if let Ok(mut profile) = profile_cell().lock() {
        add_duration_us(&mut profile.normalized_number_us, ns);
    }
}

pub fn increment_token_count() {
    if let Ok(mut profile) = profile_cell().lock() {
        increment_counter(&mut profile.token_count);
    }
}

pub fn increment_event_count() {
    if let Ok(mut profile) = profile_cell().lock() {
        increment_counter(&mut profile.event_count);
    }
}

// ---------------------------------------------------------------------------
// TokenSpanCollector / ErrorSpanCollector
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenSpanCollector {
    spans: Vec<TokenSpan>,
}

impl TokenSpanCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, spans: Vec<TokenSpan>) {
        self.spans = spans;
    }

    pub fn take(&mut self) -> Vec<TokenSpan> {
        std::mem::take(&mut self.spans)
    }

    pub fn spans(&self) -> &[TokenSpan] {
        &self.spans
    }

    fn collect(&mut self, tok: &Token, token_type: u32) {
        self.spans.push(TokenSpan {
            start_row: tok.span.start.line,
            start_col: tok.span.start.column,
            end_row: tok.span.end.line,
            end_col: tok.span.end.column,
            token_type,
        });
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ErrorSpanCollector {
    spans: Vec<ErrorSpan>,
}

impl ErrorSpanCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, spans: Vec<ErrorSpan>) {
        self.spans = spans;
    }

    pub fn take(&mut self) -> Vec<ErrorSpan> {
        std::mem::take(&mut self.spans)
    }

    pub fn spans(&self) -> &[ErrorSpan] {
        &self.spans
    }

    fn collect(&mut self, span: &Span) {
        self.spans.push(ErrorSpan {
            start_row: span.start.line,
            start_col: span.start.column,
            end_row: span.end.line,
            end_col: span.end.column,
            kind: 1,
        });
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    Object,
    Array,
}

/// and cast to/from the enum via `@enumFromInt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ObjPhase {
    KeyOrEnd = 0,
    Key = 1,
    Colon = 2,
    Value = 3,
    CommaOrEnd = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ArrPhase {
    ValueOrEnd = 0,
    Value = 1,
    CommaOrEnd = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Container {
    kind: ContainerKind,
    /// Raw u8 encoding of either ObjPhase or ArrPhase.
    phase: u8,
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathKind {
    Object,
    Array,
}

#[derive(Debug, Clone)]
struct PathFrame {
    kind: PathKind,
    index: usize,
    pending_key: Option<String>,
}

impl PathFrame {
    fn set_pending_key(&mut self, key: String) -> Result<(), JsonStreamError> {
        if self.pending_key.is_some() {
            return Err(JsonStreamError::UnexpectedToken {
                offset: 0,
                found: '\0',
            });
        }
        self.pending_key = Some(key);
        Ok(())
    }

    fn clear_pending_key(&mut self) {
        self.pending_key = None;
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recovery {
    None,
    Root,
    Object,
    Array,
}

// ---------------------------------------------------------------------------
// StreamingParser – incremental, feed-driven JSON parser
// ---------------------------------------------------------------------------

// SAFETY: CallbackSink holds a raw pointer to a sink and a function pointer.
// The pointer is only used within the lifetime of the borrow that created it,
// and is never sent across threads independently.
unsafe impl Send for CallbackSink {}

#[derive(Debug, Clone, Copy)]
struct CallbackSink {
    ctx: *mut (),
    emit: unsafe fn(*mut (), StreamingEvent) -> Result<(), CoreError>,
}

impl CallbackSink {
    fn new<T: EventSink<Error = CoreError>>(sink: &mut T) -> Self {
        Self {
            ctx: sink as *mut T as *mut (),
            emit: emit_to_sink::<T>,
        }
    }

    unsafe fn emit(self, event: StreamingEvent) -> Result<(), CoreError> {
        unsafe { (self.emit)(self.ctx, event) }
    }
}

unsafe fn emit_to_sink<T: EventSink<Error = CoreError>>(
    ctx: *mut (),
    event: StreamingEvent,
) -> Result<(), CoreError> {
    // SAFETY: `ctx` was created from `&mut T` in `CallbackSink::new` and remains
    // valid for the lifetime guaranteed by the caller that owns the sink.
    let sink = unsafe { &mut *(ctx as *mut T) };
    sink.emit(event)
}

#[derive(Debug, Clone)]
enum ParserSink {
    Buffered(Vec<StreamingEvent>),
    Event(CallbackSink),
}

///
/// Callers push chunks via [`feed`](StreamingParser::feed) and finalise via
/// [`finish`](StreamingParser::finish).  The parser uses the incremental
/// [`Scanner`] to tokenise the input and maintains a container stack with
/// `ObjPhase`/`ArrPhase` state machines so that events are emitted as soon
/// as tokens become available — no buffering of the entire document is
/// required.
///
/// When an unexpected token is encountered the parser enters a *recovery*
/// mode: it skips tokens until it finds a synchronisation point (comma,
/// closing brace/bracket) and emits [`StreamingEvent::ParseError`] events
/// into the stream.  [`TokenSpan`]s and [`ErrorSpan`]s are collected in
/// real-time during parsing.
#[derive(Debug, Clone)]
pub struct StreamingParser {
    sink: ParserSink,

    sink_error: Option<CoreError>,

    event_count: usize,

    /// Incremental scanner.
    scanner: Scanner,

    /// Whether to attempt nested-JSON expansion on string values.
    nest_json: bool,

    /// Current nesting depth for nested-JSON (capped at 8).
    nest_depth: u8,

    /// Canonical source replacements discovered while parsing complete tokens.
    source_rewrites: Vec<SourceRewrite>,

    /// True once a JSON string value has been expanded into nested events.
    nested_json_expanded: bool,

    /// Whether to emit JSON-path strings in event metadata.
    emit_path: bool,

    /// When true, skip number normalisation (used by `for_view`).
    skip_number_normalization: bool,

    /// Container stack – each entry is an open object or array.
    stack: Vec<Container>,

    /// Path stack – mirrors the container stack for path construction.
    path_stack: Vec<PathFrame>,

    /// Single-token lookahead (used by `peekNonCommentToken`).
    lookahead: Option<Token>,

    // ---- document lifecycle ----
    doc_started: bool,
    doc_ended: bool,
    root_done: bool,
    saw_error: bool,

    /// Span of the last non-EOF token (used for end-of-input error reporting).
    last_token_span: Option<Span>,

    /// Current recovery mode.
    recovery: Recovery,

    // ---- collectors (populated in real-time) ----
    token_span_collector: TokenSpanCollector,
    error_span_collector: ErrorSpanCollector,

    /// Set to true after `finish()` has been called.
    finished: bool,
}

impl Default for StreamingParser {
    fn default() -> Self {
        Self {
            sink: ParserSink::Buffered(Vec::new()),
            sink_error: None,
            event_count: 0,
            scanner: Scanner::new(None, false, None),
            nest_json: false,
            nest_depth: 0,
            source_rewrites: Vec::new(),
            nested_json_expanded: false,
            emit_path: false,
            skip_number_normalization: false,
            stack: Vec::new(),
            path_stack: Vec::new(),
            lookahead: None,
            doc_started: false,
            doc_ended: false,
            root_done: false,
            saw_error: false,
            last_token_span: None,
            recovery: Recovery::None,
            token_span_collector: TokenSpanCollector::new(),
            error_span_collector: ErrorSpanCollector::new(),
            finished: false,
        }
    }
}

impl StreamingParser {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    pub fn new(nest_json: bool) -> Self {
        Self {
            nest_json,
            ..Self::default()
        }
    }

    pub fn with_path_emission(nest_json: bool, emit_path: bool) -> Self {
        Self {
            nest_json,
            emit_path,
            ..Self::default()
        }
    }

    pub fn with_builder(nest_json: bool, emit_path: bool, builder: &mut Builder) -> Self {
        Self::with_sink(nest_json, emit_path, builder)
    }

    pub fn with_sink<T: EventSink<Error = CoreError>>(
        nest_json: bool,
        emit_path: bool,
        sink: &mut T,
    ) -> Self {
        Self {
            sink: ParserSink::Event(CallbackSink::new(sink)),
            nest_json,
            emit_path,
            ..Self::default()
        }
    }

    /// Create a parser for "view" mode: no path emission, skip number
    /// normalisation.
    pub fn for_view(nest_json: bool) -> Self {
        Self {
            nest_json,
            skip_number_normalization: true,
            ..Self::default()
        }
    }

    // ------------------------------------------------------------------
    // Scanner configuration (profile hooks, token cache)
    // ------------------------------------------------------------------

    /// Install [`ProfileHooks`] on the underlying scanner so that
    /// fine-grained timing callbacks are invoked during tokenisation.
    pub fn set_profile_hooks(&mut self, hooks: ProfileHooks) {
        // Re-create the scanner with the hooks, preserving existing state
        // that matters: token cache and compact threshold.
        let cache = self.scanner.token_cache().cloned();
        let cache_enabled = self.scanner.token_cache_enabled();
        let compact_threshold = self.scanner.compact_threshold();
        self.scanner = Scanner::new(cache, cache_enabled, Some(hooks))
            .with_compact_threshold(compact_threshold);
    }

    /// Enable the token cache on the underlying scanner.
    ///
    /// When enabled, the scanner will intern decoded string and number
    /// values so that repeated occurrences share a single allocation.
    pub fn set_token_cache(&mut self, cache: TokenCache) {
        let hooks = self.scanner.take_profile_hooks();
        let compact_threshold = self.scanner.compact_threshold();
        self.scanner =
            Scanner::new(Some(cache), true, hooks).with_compact_threshold(compact_threshold);
    }

    /// Return a reference to the scanner's token cache, if any.
    pub fn token_cache(&self) -> Option<&TokenCache> {
        self.scanner.token_cache()
    }

    /// Set the current nesting depth for nested-JSON expansion.
    ///
    /// Callers use this when a parse starts from inside an already-expanded
    /// nested JSON value so recursive expansion still respects the global cap.
    pub fn set_nest_depth(&mut self, depth: u8) {
        self.nest_depth = depth.min(8);
    }

    pub fn take_source_rewrites(&mut self) -> Vec<SourceRewrite> {
        std::mem::take(&mut self.source_rewrites)
    }

    pub fn nested_json_expanded(&self) -> bool {
        self.nested_json_expanded
    }

    // ------------------------------------------------------------------
    // Feed / Finish (public API)
    // ------------------------------------------------------------------

    /// Feed a chunk of JSON text into the parser.
    ///
    /// The parser incrementally tokenises the chunk and emits events for
    /// every complete token.  Partial tokens are held in the scanner until
    /// more data arrives.
    pub fn feed(&mut self, chunk: &str) -> Result<(), JsonStreamError> {
        if self.finished {
            return Err(JsonStreamError::TrailingCharacters {
                offset: self.scanner.total_fed(),
            });
        }
        self.scanner.feed(chunk);
        self.parse_available(false)?;
        if self.sink_error.is_some() {
            return Err(JsonStreamError::UnexpectedEnd);
        }
        Ok(())
    }

    pub fn feed_bytes(&mut self, chunk: &[u8]) -> Result<(), JsonStreamError> {
        if self.finished {
            return Err(JsonStreamError::TrailingCharacters {
                offset: self.scanner.total_fed(),
            });
        }
        self.scanner.feed_bytes(chunk);
        self.parse_available(false)?;
        if self.sink_error.is_some() {
            return Err(JsonStreamError::UnexpectedEnd);
        }
        Ok(())
    }

    /// Signal that no more chunks will be fed and finalise parsing.
    ///
    /// Returns the complete list of streaming events.
    pub(crate) fn finish_without_events(&mut self) -> Result<(), JsonStreamError> {
        self.parse_available(true)?;

        if self.sink_error.is_some() {
            self.finished = true;
            return Err(JsonStreamError::UnexpectedEnd);
        }

        if !self.root_done || !self.stack.is_empty() {
            self.emit_unexpected_end_of_input();
            self.finished = true;
            return Err(JsonStreamError::UnexpectedEnd);
        }

        if self.saw_error {
            self.finished = true;
            return Err(JsonStreamError::UnexpectedEnd);
        }

        if !self.doc_ended {
            self.emit_event(StreamingEvent::DocEnd(Meta {
                document: 0,
                ..Meta::default()
            }));
            self.doc_ended = true;
        }

        record_decode_profile(
            "",
            self.event_count,
            self.token_span_collector.spans().len(),
            self.error_span_collector.spans().len(),
        );

        self.finished = true;
        Ok(())
    }

    /// Signal that no more chunks will be fed and finalise parsing.
    ///
    /// Returns the complete list of streaming events.
    pub fn finish(&mut self) -> Result<Vec<StreamingEvent>, JsonStreamError> {
        self.finish_without_events()?;
        Ok(self.take_events())
    }

    /// Drain currently emitted events without finalising the parser.
    pub fn take_events(&mut self) -> Vec<StreamingEvent> {
        match &mut self.sink {
            ParserSink::Buffered(events) => std::mem::take(events),
            ParserSink::Event(_) => Vec::new(),
        }
    }

    /// Convenience: finish and build a document tree.
    pub fn finish_to_document(&mut self) -> Result<DecodedDocument, CoreError> {
        let mut builder = Builder::new();
        self.finish_to_builder(&mut builder)?;
        builder.into_document()
    }

    /// Finish and return the document together with token/error spans.
    pub fn finish_with_token_spans(&mut self) -> Result<DecodeWithTokenSpansResult, CoreError> {
        let mut builder = Builder::new();
        self.finish_to_builder(&mut builder)?;
        let document = builder.into_document()?;
        Ok(DecodeWithTokenSpansResult {
            document,
            token_spans: self.token_span_collector.spans().to_vec(),
            error_spans: self.error_span_collector.spans().to_vec(),
        })
    }

    /// Take accumulated token spans (clears the internal collector).
    pub fn take_token_spans(&mut self) -> Vec<TokenSpan> {
        self.token_span_collector.take()
    }

    /// Take accumulated error spans (clears the internal collector).
    pub fn take_error_spans(&mut self) -> Vec<ErrorSpan> {
        self.error_span_collector.take()
    }

    // ==================================================================
    // Core parse loop
    // ==================================================================

    /// Pull tokens from the scanner and drive the state machine until the
    /// scanner stalls or the document is complete.
    fn parse_available(&mut self, final_chunk: bool) -> Result<(), JsonStreamError> {
        if !self.doc_started {
            self.emit_event(StreamingEvent::DocStart(Meta {
                document: 0,
                ..Meta::default()
            }));
            self.doc_started = true;
        }

        loop {
            // --- recovery path ---
            if self.recovery != Recovery::None {
                let progressed = self.recover(final_chunk)?;
                if !progressed {
                    return Ok(());
                }
                continue;
            }

            // --- normal path ---
            let tok = match self.next_non_comment_token(final_chunk)? {
                Some(t) => t,
                None => return Ok(()),
            };

            if tok.tag != TokenTag::Eof {
                self.last_token_span = Some(tok.span);
            }

            if tok.tag == TokenTag::Eof {
                if final_chunk && self.root_done && !self.doc_ended {
                    self.emit_event(StreamingEvent::DocEnd(Meta {
                        document: 0,
                        ..Meta::default()
                    }));
                    self.doc_ended = true;
                }
                return Ok(());
            }

            if tok.tag == TokenTag::Invalid {
                self.emit_error(&tok.span, "invalid token");
                self.recovery = Recovery::Root;
                continue;
            }

            if self.root_done {
                self.emit_error(&tok.span, "unexpected trailing token");
                self.recovery = Recovery::Root;
                continue;
            }

            // Collect token span in real-time.
            if let Some(tt) = self.classify_token_type(&tok) {
                self.token_span_collector.collect(&tok, tt);
            }

            if self.stack.is_empty() {
                self.parse_value_token(&tok, None)?;
                continue;
            }

            let top = self.stack.last().copied();
            match top {
                Some(Container {
                    kind: ContainerKind::Object,
                    ..
                }) => self.step_object(tok, final_chunk)?,
                Some(Container {
                    kind: ContainerKind::Array,
                    ..
                }) => self.step_array(tok, final_chunk)?,
                None => {} // unreachable – handled above
            }
        }
    }

    // ------------------------------------------------------------------
    // Object state machine
    // ------------------------------------------------------------------

    fn step_object(&mut self, tok: Token, final_chunk: bool) -> Result<(), JsonStreamError> {
        let top = self.stack.last().copied().unwrap();
        let phase = unsafe { std::mem::transmute::<u8, ObjPhase>(top.phase) };

        match phase {
            ObjPhase::KeyOrEnd => match tok.tag {
                TokenTag::RightBrace => self.close_object(&tok.span)?,
                TokenTag::String => {
                    self.emit_map_key(&tok)?;
                    self.set_top_phase(ObjPhase::Colon as u8);
                }
                _ => self.unexpected_token(&tok, Recovery::Object)?,
            },
            ObjPhase::Key => match tok.tag {
                TokenTag::String => {
                    self.emit_map_key(&tok)?;
                    self.set_top_phase(ObjPhase::Colon as u8);
                }
                _ => self.unexpected_token(&tok, Recovery::Object)?,
            },
            ObjPhase::Colon => match tok.tag {
                TokenTag::Colon => {
                    self.set_top_phase(ObjPhase::Value as u8);
                }
                _ => self.unexpected_token(&tok, Recovery::Object)?,
            },
            ObjPhase::Value => {
                self.parse_value_token(&tok, None)?;
            }
            ObjPhase::CommaOrEnd => match tok.tag {
                TokenTag::Comma => {
                    // Peek ahead to see if there's a key coming.
                    let _next = self.peek_non_comment_token(final_chunk)?;
                    self.set_top_phase(ObjPhase::Key as u8);
                }
                TokenTag::RightBrace => self.close_object(&tok.span)?,
                _ => self.unexpected_token(&tok, Recovery::Object)?,
            },
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Array state machine
    // ------------------------------------------------------------------

    fn step_array(&mut self, tok: Token, final_chunk: bool) -> Result<(), JsonStreamError> {
        let top = self.stack.last().copied().unwrap();
        let phase = unsafe { std::mem::transmute::<u8, ArrPhase>(top.phase) };

        match phase {
            ArrPhase::ValueOrEnd => match tok.tag {
                TokenTag::RightBracket => self.close_array(&tok.span)?,
                _ => {
                    self.parse_value_token(&tok, None)?;
                }
            },
            ArrPhase::Value => {
                self.parse_value_token(&tok, None)?;
            }
            ArrPhase::CommaOrEnd => match tok.tag {
                TokenTag::Comma => {
                    let _next = self.peek_non_comment_token(final_chunk)?;
                    self.set_top_phase(ArrPhase::Value as u8);
                }
                TokenTag::RightBracket => self.close_array(&tok.span)?,
                _ => self.unexpected_token(&tok, Recovery::Array)?,
            },
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Value dispatch
    // ------------------------------------------------------------------

    fn parse_value_token(
        &mut self,
        tok: &Token,
        forced_path: Option<&str>,
    ) -> Result<(), JsonStreamError> {
        match tok.tag {
            TokenTag::LeftBrace => {
                let meta = self.meta_from_span(&tok.span, SemType::Map, forced_path);
                self.emit_event(StreamingEvent::MapStart(meta));
                self.stack.push(Container {
                    kind: ContainerKind::Object,
                    phase: ObjPhase::KeyOrEnd as u8,
                });
                self.path_stack.push(PathFrame {
                    kind: PathKind::Object,
                    index: 0,
                    pending_key: None,
                });
            }
            TokenTag::LeftBracket => {
                let meta = self.meta_from_span(&tok.span, SemType::Seq, forced_path);
                self.emit_event(StreamingEvent::SeqStart(meta));
                self.stack.push(Container {
                    kind: ContainerKind::Array,
                    phase: ArrPhase::ValueOrEnd as u8,
                });
                self.path_stack.push(PathFrame {
                    kind: PathKind::Array,
                    index: 0,
                    pending_key: None,
                });
            }
            TokenTag::String => self.emit_string_scalar(tok, forced_path)?,
            TokenTag::Number => self.emit_number_scalar(tok, forced_path)?,
            TokenTag::TrueKw => {
                self.emit_keyword_scalar(tok, SemType::Boolean, "true", forced_path)?
            }
            TokenTag::FalseKw => {
                self.emit_keyword_scalar(tok, SemType::Boolean, "false", forced_path)?
            }
            TokenTag::NullKw => self.emit_keyword_scalar(tok, SemType::Nil, "null", forced_path)?,
            _ => {
                let scope = if self.stack.is_empty() {
                    Recovery::Root
                } else {
                    Recovery::Array
                };
                self.unexpected_token(tok, scope)?;
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Event emitters
    // ------------------------------------------------------------------

    fn emit_map_key(&mut self, tok: &Token) -> Result<(), JsonStreamError> {
        let path = self.path_for_key(&tok.value);
        let meta = self.meta_from_span_with_path(&tok.span, SemType::Str, &path);
        let frame = self
            .current_path_frame_mut()
            .ok_or(JsonStreamError::UnexpectedEnd)?;
        frame.set_pending_key(tok.value.clone())?;
        self.emit_event(StreamingEvent::MapKey {
            value: tok.value.clone(),
            meta,
        });
        Ok(())
    }

    fn emit_string_scalar(
        &mut self,
        tok: &Token,
        forced_path: Option<&str>,
    ) -> Result<(), JsonStreamError> {
        if self.try_emit_nested_json_scalar(tok, forced_path)? {
            return Ok(());
        }
        let meta = self.meta_from_span(&tok.span, SemType::Str, forced_path);
        self.emit_event(StreamingEvent::Scalar {
            value: tok.value.clone(),
            meta,
        });
        self.on_value_complete();
        Ok(())
    }

    fn try_emit_nested_json_scalar(
        &mut self,
        tok: &Token,
        forced_path: Option<&str>,
    ) -> Result<bool, JsonStreamError> {
        if !self.nest_json
            || self.nest_depth >= 8
            || !super::nested_json::is_nested_json_candidate(&tok.value)
        {
            return Ok(false);
        }

        let trimmed = super::nested_json::trim_ascii_whitespace(&tok.value);
        let nested = match decode_nested_materialized(
            trimmed,
            self.nest_depth.saturating_add(1),
            self.emit_path,
        ) {
            Ok(nested) => nested,
            Err(_) => return Ok(false),
        };

        let outer_path = if let Some(path) = forced_path {
            path.to_owned()
        } else if self.emit_path {
            self.build_path(None)
        } else {
            String::new()
        };
        let outer_start = tok.span.start.offset as u32;
        let outer_end = tok.span.end.offset as u32;

        self.source_rewrites.push(SourceRewrite {
            start_byte: outer_start,
            old_end_byte: outer_end,
            replacement: nested.source,
        });
        self.nested_json_expanded = true;

        for event in nested.events {
            if let Some(event) = rebase_nested_event_for_rewrite(event, outer_start, &outer_path) {
                self.emit_event(event);
            }
        }

        self.on_value_complete();
        Ok(true)
    }

    fn emit_number_scalar(
        &mut self,
        tok: &Token,
        forced_path: Option<&str>,
    ) -> Result<(), JsonStreamError> {
        if self.skip_number_normalization {
            let sem_type = if tok.value.contains(|c: char| matches!(c, '.' | 'e' | 'E')) {
                SemType::Float
            } else {
                SemType::Int
            };
            let meta = self.meta_from_span(&tok.span, sem_type, forced_path);
            self.emit_event(StreamingEvent::Scalar {
                value: tok.value.clone(),
                meta,
            });
            self.on_value_complete();
            return Ok(());
        }

        let normalized = parse_tree::normalized_number_value(&tok.value);
        let meta = self.meta_from_span(&tok.span, normalized.sem_type, forced_path);
        self.emit_event(StreamingEvent::Scalar {
            value: normalized.value,
            meta,
        });
        self.on_value_complete();
        Ok(())
    }

    fn emit_keyword_scalar(
        &mut self,
        tok: &Token,
        sem_type: SemType,
        literal: &str,
        forced_path: Option<&str>,
    ) -> Result<(), JsonStreamError> {
        let meta = self.meta_from_span(&tok.span, sem_type, forced_path);
        self.emit_event(StreamingEvent::Scalar {
            value: literal.to_string(),
            meta,
        });
        self.on_value_complete();
        Ok(())
    }

    // ------------------------------------------------------------------
    // Container close helpers
    // ------------------------------------------------------------------

    fn close_object(&mut self, span: &Span) -> Result<(), JsonStreamError> {
        let meta = self.meta_from_span(span, SemType::Map, None);
        self.emit_event(StreamingEvent::MapEnd(meta));
        self.stack.pop();
        self.pop_path_frame();
        self.on_value_complete();
        Ok(())
    }

    fn close_array(&mut self, span: &Span) -> Result<(), JsonStreamError> {
        let meta = self.meta_from_span(span, SemType::Seq, None);
        self.emit_event(StreamingEvent::SeqEnd(meta));
        self.stack.pop();
        self.pop_path_frame();
        self.on_value_complete();
        Ok(())
    }

    // ------------------------------------------------------------------
    // Value-complete bookkeeping
    // ------------------------------------------------------------------

    fn on_value_complete(&mut self) {
        if self.stack.is_empty() {
            self.root_done = true;
            return;
        }
        let top = self.stack.last_mut().unwrap();
        match top.kind {
            ContainerKind::Object => top.phase = ObjPhase::CommaOrEnd as u8,
            ContainerKind::Array => top.phase = ArrPhase::CommaOrEnd as u8,
        }
        if let Some(frame) = self.current_path_frame_mut() {
            match frame.kind {
                PathKind::Object => frame.clear_pending_key(),
                PathKind::Array => frame.index += 1,
            }
        }
    }

    // ------------------------------------------------------------------
    // Path helpers
    // ------------------------------------------------------------------

    fn current_path_frame_mut(&mut self) -> Option<&mut PathFrame> {
        self.path_stack.last_mut()
    }

    fn pop_path_frame(&mut self) {
        if let Some(mut frame) = self.path_stack.pop() {
            frame.clear_pending_key();
        }
    }

    fn path_for_key(&self, key: &str) -> String {
        if !self.emit_path {
            return String::new();
        }
        self.build_path(Some(key))
    }

    fn build_path(&self, maybe_key: Option<&str>) -> String {
        let mut out = String::from("$");
        for frame in &self.path_stack {
            match frame.kind {
                PathKind::Object => {
                    if let Some(ref k) = frame.pending_key {
                        append_key_path(&mut out, k);
                    }
                }
                PathKind::Array => {
                    append_index_path(&mut out, frame.index);
                }
            }
        }
        if let Some(k) = maybe_key {
            append_key_path(&mut out, k);
        }
        out
    }

    // ------------------------------------------------------------------
    // Metadata construction
    // ------------------------------------------------------------------

    fn meta_from_span(&self, span: &Span, sem_type: SemType, forced_path: Option<&str>) -> Meta {
        let path = if let Some(p) = forced_path {
            p.to_string()
        } else if self.emit_path {
            self.build_path(None)
        } else {
            String::new()
        };
        Meta {
            tag: sem_type.tag().to_string(),
            sem_type: Some(sem_type),
            start_byte: span.start.offset as u32,
            end_byte: span.end.offset as u32,
            line: span.start.line.wrapping_add(1) as i32,
            column: span.start.column.wrapping_add(1) as i32,
            path,
            ..Meta::default()
        }
    }

    fn meta_from_span_with_path(&self, span: &Span, sem_type: SemType, path: &str) -> Meta {
        Meta {
            tag: sem_type.tag().to_string(),
            sem_type: Some(sem_type),
            start_byte: span.start.offset as u32,
            end_byte: span.end.offset as u32,
            line: span.start.line.wrapping_add(1) as i32,
            column: span.start.column.wrapping_add(1) as i32,
            path: path.to_string(),
            ..Meta::default()
        }
    }

    fn classify_token_type(&self, tok: &Token) -> Option<u32> {
        let is_object_key = tok.tag == TokenTag::String && !self.stack.is_empty() && {
            let top = self.stack.last().unwrap();
            top.kind == ContainerKind::Object && {
                let phase: ObjPhase = unsafe { std::mem::transmute(top.phase) };
                phase == ObjPhase::KeyOrEnd || phase == ObjPhase::Key
            }
        };

        match tok.tag {
            TokenTag::String => {
                if is_object_key {
                    Some(TOKEN_TYPE_KEY)
                } else {
                    Some(TOKEN_TYPE_STRING)
                }
            }
            TokenTag::Number => Some(TOKEN_TYPE_INT),
            TokenTag::TrueKw | TokenTag::FalseKw => Some(TOKEN_TYPE_BOOLEAN),
            TokenTag::NullKw => Some(TOKEN_TYPE_NULL),
            TokenTag::LeftBrace
            | TokenTag::RightBrace
            | TokenTag::LeftBracket
            | TokenTag::RightBracket
            | TokenTag::Comma
            | TokenTag::Colon => Some(TOKEN_TYPE_PUNCTUATION),
            TokenTag::Comment => Some(TOKEN_TYPE_COMMENT),
            _ => None,
        }
    }

    // ------------------------------------------------------------------
    // Error handling & recovery
    // ------------------------------------------------------------------

    fn unexpected_token(&mut self, tok: &Token, scope: Recovery) -> Result<(), JsonStreamError> {
        self.emit_error(&tok.span, "unexpected token");
        self.recovery = scope;
        Ok(())
    }

    fn emit_error(&mut self, span: &Span, message: &str) {
        self.saw_error = true;
        self.error_span_collector.collect(span);

        let path = if self.emit_path {
            self.build_path(None)
        } else {
            String::new()
        };

        let meta = Meta {
            start_byte: span.start.offset as u32,
            end_byte: span.end.offset as u32,
            line: span.start.line.wrapping_add(1) as i32,
            column: span.start.column.wrapping_add(1) as i32,
            path,
            ..Meta::default()
        };

        self.emit_event(StreamingEvent::ParseError {
            message: message.to_string(),
            meta,
        });
    }

    fn emit_unexpected_end_of_input(&mut self) {
        let span = self.last_token_span.unwrap_or(Span {
            start: self.scanner.position(),
            end: self.scanner.position(),
        });
        self.emit_error(&span, "unexpected end of input");
    }

    /// Attempt to recover from an unexpected token by skipping ahead to a
    /// synchronisation point.  Returns `true` if progress was made (a token
    /// was consumed), `false` if the scanner stalled.
    fn recover(&mut self, final_chunk: bool) -> Result<bool, JsonStreamError> {
        let tok = match self.next_non_comment_token(final_chunk)? {
            Some(t) => t,
            None => return Ok(false),
        };

        if tok.tag == TokenTag::Eof {
            self.recovery = Recovery::None;
            return Ok(true);
        }

        match self.recovery {
            Recovery::Root => {
                if tok.tag == TokenTag::RightBrace || tok.tag == TokenTag::RightBracket {
                    self.recovery = Recovery::None;
                }
            }
            Recovery::Object => {
                if tok.tag == TokenTag::Comma {
                    if let Some(top) = self.stack.last_mut() {
                        top.phase = ObjPhase::Key as u8;
                    }
                    self.recovery = Recovery::None;
                } else if tok.tag == TokenTag::RightBrace {
                    self.close_object(&tok.span)?;
                    self.recovery = Recovery::None;
                }
            }
            Recovery::Array => {
                if tok.tag == TokenTag::Comma {
                    if let Some(top) = self.stack.last_mut() {
                        top.phase = ArrPhase::Value as u8;
                    }
                    self.recovery = Recovery::None;
                } else if tok.tag == TokenTag::RightBracket {
                    self.close_array(&tok.span)?;
                    self.recovery = Recovery::None;
                }
            }
            Recovery::None => {}
        }

        Ok(true)
    }

    // ------------------------------------------------------------------
    // Scanner helpers
    // ------------------------------------------------------------------

    /// Get the next non-comment token, consuming the lookahead if present.
    fn next_non_comment_token(
        &mut self,
        final_chunk: bool,
    ) -> Result<Option<Token>, JsonStreamError> {
        if let Some(tok) = self.lookahead.take() {
            return Ok(Some(tok));
        }
        let tok = self.scanner.next_token(final_chunk);
        if tok.tag == TokenTag::None {
            return Ok(None);
        }
        Ok(Some(tok))
    }

    /// Peek at the next non-comment token without consuming it.
    fn peek_non_comment_token(
        &mut self,
        final_chunk: bool,
    ) -> Result<Option<Token>, JsonStreamError> {
        if let Some(ref tok) = self.lookahead {
            return Ok(Some(tok.clone()));
        }
        let tok = self.scanner.next_token(final_chunk);
        if tok.tag == TokenTag::None {
            return Ok(None);
        }
        self.lookahead = Some(tok.clone());
        Ok(Some(tok))
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn set_top_phase(&mut self, phase: u8) {
        if let Some(top) = self.stack.last_mut() {
            top.phase = phase;
        }
    }

    fn connect_builder(&mut self, builder: &mut Builder) -> Result<(), CoreError> {
        self.connect_sink(builder)
    }

    pub fn finish_to_builder(&mut self, builder: &mut Builder) -> Result<(), CoreError> {
        self.connect_builder(builder)?;
        self.finish_without_events()
            .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
        self.take_sink_error()
    }

    fn connect_sink<T: EventSink<Error = CoreError>>(
        &mut self,
        sink: &mut T,
    ) -> Result<(), CoreError> {
        let pending_events = match &mut self.sink {
            ParserSink::Buffered(events) => std::mem::take(events),
            ParserSink::Event(_) => {
                return Err(CoreError::Parse(ParseError::InvalidSyntax));
            }
        };

        for event in pending_events {
            sink.emit(event)?;
        }

        self.sink = ParserSink::Event(CallbackSink::new(sink));
        Ok(())
    }

    fn take_sink_error(&mut self) -> Result<(), CoreError> {
        match self.sink_error.take() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn emit_event(&mut self, event: StreamingEvent) {
        let start_ns = monotonic_now_ns();
        match &mut self.sink {
            ParserSink::Buffered(events) => events.push(event),
            ParserSink::Event(sink) => {
                if self.sink_error.is_none() {
                    if let Err(err) = unsafe { sink.emit(event) } {
                        self.sink_error = Some(err);
                    }
                }
            }
        }
        self.event_count = self.event_count.saturating_add(1);
        add_sink_emit_ns(monotonic_now_ns() - start_ns);
        increment_event_count();
    }
}

// ---------------------------------------------------------------------------
// StreamDecoder – thin wrapper around StreamingParser with TokenCache support
// ---------------------------------------------------------------------------

/// A streaming JSON decoder that wraps [`StreamingParser`] and optionally
/// enables a [`TokenCache`] for string/number interning.
///
/// parser, and token cache, wiring them together.
#[derive(Debug)]
pub struct StreamDecoder {
    parser: StreamingParser,
    /// Owned token cache (kept alive for the lifetime of the decoder).
    token_cache: Option<TokenCache>,
}

impl Default for StreamDecoder {
    fn default() -> Self {
        Self {
            parser: StreamingParser::default(),
            token_cache: None,
        }
    }
}

impl Clone for StreamDecoder {
    fn clone(&self) -> Self {
        Self {
            parser: self.parser.clone(),
            token_cache: self.token_cache.clone(),
        }
    }
}

impl StreamDecoder {
    pub fn new(nest_json: bool) -> Self {
        Self {
            parser: StreamingParser::new(nest_json),
            token_cache: None,
        }
    }

    pub fn with_path_emission(nest_json: bool, emit_path: bool) -> Self {
        Self {
            parser: StreamingParser::with_path_emission(nest_json, emit_path),
            token_cache: None,
        }
    }

    pub fn for_view(nest_json: bool) -> Self {
        Self {
            parser: StreamingParser::for_view(nest_json),
            token_cache: None,
        }
    }

    /// Create a decoder with token caching enabled.
    ///
    /// When enabled, the scanner will intern decoded string and number
    /// values so that repeated occurrences share a single allocation.
    pub fn with_token_cache(nest_json: bool) -> Self {
        let mut parser = StreamingParser::new(nest_json);
        let cache = TokenCache::new();
        parser.set_token_cache(cache.clone());
        Self {
            parser,
            token_cache: Some(cache),
        }
    }

    /// Create a decoder with path emission and token caching enabled.
    pub fn with_path_emission_and_cache(nest_json: bool, emit_path: bool) -> Self {
        let mut parser = StreamingParser::with_path_emission(nest_json, emit_path);
        let cache = TokenCache::new();
        parser.set_token_cache(cache.clone());
        Self {
            parser,
            token_cache: Some(cache),
        }
    }

    pub fn take_source_rewrites(&mut self) -> Vec<SourceRewrite> {
        self.parser.take_source_rewrites()
    }

    pub fn feed(&mut self, chunk: &str) -> Result<(), JsonStreamError> {
        self.parser.feed(chunk)
    }

    pub fn feed_bytes(&mut self, chunk: &[u8]) -> Result<(), JsonStreamError> {
        self.parser.feed_bytes(chunk)
    }

    pub fn finish(&mut self) -> Result<DecodedDocument, CoreError> {
        self.parser.finish_to_document()
    }

    pub fn finish_with_token_spans(&mut self) -> Result<DecodeWithTokenSpansResult, CoreError> {
        self.parser.finish_with_token_spans()
    }

    pub fn take_token_spans(&mut self) -> Vec<TokenSpan> {
        self.parser.take_token_spans()
    }

    pub fn take_error_spans(&mut self) -> Vec<ErrorSpan> {
        self.parser.take_error_spans()
    }

    /// Set profile hooks on the underlying scanner.
    pub fn set_profile_hooks(&mut self, hooks: ProfileHooks) {
        self.parser.set_profile_hooks(hooks);
    }

    pub fn set_nest_depth(&mut self, depth: u8) {
        self.parser.set_nest_depth(depth);
    }

    pub fn nested_json_expanded(&self) -> bool {
        self.parser.nested_json_expanded()
    }
    /// Drain accumulated streaming events without finalising the parser.
    /// Useful for incremental feed-then-drain cycles where events are
    /// consumed by an external Builder between chunks.
    pub fn take_events(&mut self) -> Vec<StreamingEvent> {
        self.parser.take_events()
    }

    pub fn finish_events(&mut self) -> Result<Vec<StreamingEvent>, JsonStreamError> {
        self.parser.finish()
    }

    /// Finalise parsing, routing all remaining events to `builder`.
    /// After this call the parser is finished; call `builder.into_document()`
    /// to obtain the final document tree.
    pub fn finish_to_builder(&mut self, builder: &mut Builder) -> Result<(), CoreError> {
        self.parser.finish_to_builder(builder)
    }
}

pub type Decoder = StreamDecoder;

// ---------------------------------------------------------------------------
// Path helpers (free functions)
// ---------------------------------------------------------------------------

fn append_key_path(out: &mut String, key: &str) {
    out.push('.');
    out.push_str(key);
}

fn decode_nested_materialized(
    input: &str,
    nest_depth: u8,
    emit_path: bool,
) -> Result<NestedMaterialization, JsonStreamError> {
    let mut parser = StreamingParser::with_path_emission(true, emit_path);
    parser.set_nest_depth(nest_depth);
    parser.feed(input)?;
    let events = parser.finish()?;
    let rewrites = parser.take_source_rewrites();
    let (source, events) = materialize_source_rewrites(input, events, rewrites)?;
    Ok(NestedMaterialization { source, events })
}

fn materialize_source_rewrites(
    input: &str,
    events: Vec<StreamingEvent>,
    rewrites: Vec<SourceRewrite>,
) -> Result<(String, Vec<StreamingEvent>), JsonStreamError> {
    if rewrites.is_empty() {
        return Ok((input.to_owned(), events));
    }

    let mut sorted_rewrites = rewrites;
    sorted_rewrites.sort_by_key(|rewrite| rewrite.start_byte);

    let mut source = String::with_capacity(input.len());
    let mut cursor = 0usize;
    let mut applied = Vec::with_capacity(sorted_rewrites.len());

    for rewrite in sorted_rewrites {
        let raw_start = rewrite.start_byte as usize;
        let raw_end = rewrite.old_end_byte as usize;
        if raw_start < cursor || raw_end < raw_start || raw_end > input.len() {
            return Err(JsonStreamError::UnexpectedEnd);
        }

        source.push_str(
            input
                .get(cursor..raw_start)
                .ok_or(JsonStreamError::UnexpectedEnd)?,
        );

        let source_start = source.len();
        source.push_str(&rewrite.replacement);
        let cumulative_delta_after = source.len() as i64 - raw_end as i64;
        applied.push(AppliedSourceRewrite {
            raw_start,
            raw_end,
            source_start,
            replacement_len: rewrite.replacement.len(),
            cumulative_delta_after,
        });
        cursor = raw_end;
    }

    source.push_str(input.get(cursor..).ok_or(JsonStreamError::UnexpectedEnd)?);

    let line_index = crate::core::LineIndex::build(&source);
    let rebased_events = events
        .into_iter()
        .map(|event| rebase_materialized_event(event, &applied, &line_index))
        .collect();
    Ok((source, rebased_events))
}

fn rebase_materialized_event(
    event: StreamingEvent,
    applied: &[AppliedSourceRewrite],
    line_index: &crate::core::LineIndex,
) -> StreamingEvent {
    match event {
        StreamingEvent::DocStart(meta) => {
            StreamingEvent::DocStart(rebase_materialized_meta(meta, applied, line_index))
        }
        StreamingEvent::DocEnd(meta) => {
            StreamingEvent::DocEnd(rebase_materialized_meta(meta, applied, line_index))
        }
        StreamingEvent::MapStart(meta) => {
            StreamingEvent::MapStart(rebase_materialized_meta(meta, applied, line_index))
        }
        StreamingEvent::MapKey { value, meta } => StreamingEvent::MapKey {
            value,
            meta: rebase_materialized_meta(meta, applied, line_index),
        },
        StreamingEvent::MapEnd(meta) => {
            StreamingEvent::MapEnd(rebase_materialized_meta(meta, applied, line_index))
        }
        StreamingEvent::SeqStart(meta) => {
            StreamingEvent::SeqStart(rebase_materialized_meta(meta, applied, line_index))
        }
        StreamingEvent::SeqEnd(meta) => {
            StreamingEvent::SeqEnd(rebase_materialized_meta(meta, applied, line_index))
        }
        StreamingEvent::Scalar { value, meta } => StreamingEvent::Scalar {
            value,
            meta: rebase_materialized_meta(meta, applied, line_index),
        },
        StreamingEvent::Alias { anchor, meta } => StreamingEvent::Alias {
            anchor,
            meta: rebase_materialized_meta(meta, applied, line_index),
        },
        StreamingEvent::ParseError { message, meta } => StreamingEvent::ParseError {
            message,
            meta: rebase_materialized_meta(meta, applied, line_index),
        },
    }
}

fn rebase_materialized_meta(
    mut meta: Meta,
    applied: &[AppliedSourceRewrite],
    line_index: &crate::core::LineIndex,
) -> Meta {
    meta.start_byte = map_materialized_offset(meta.start_byte, applied);
    meta.end_byte = map_materialized_offset(meta.end_byte, applied);
    let line_column = line_index.offset_to_line_column(meta.start_byte as usize);
    meta.line = line_column.line as i32 + 1;
    meta.column = line_column.column as i32 + 1;
    meta
}

fn map_materialized_offset(offset: u32, applied: &[AppliedSourceRewrite]) -> u32 {
    let raw = offset as usize;
    let index = applied.partition_point(|rewrite| rewrite.raw_start <= raw);
    if index == 0 {
        return offset;
    }
    let rewrite = &applied[index - 1];
    if raw <= rewrite.raw_end {
        let inner = raw.saturating_sub(rewrite.raw_start);
        let clamped = inner.min(rewrite.replacement_len);
        return (rewrite.source_start + clamped).min(u32::MAX as usize) as u32;
    }
    let mapped = raw as i64 + rewrite.cumulative_delta_after;
    clamp_offset_to_u32(mapped)
}

pub(crate) fn clamp_offset_to_u32(offset: i64) -> u32 {
    if offset <= 0 {
        0
    } else if offset >= u32::MAX as i64 {
        u32::MAX
    } else {
        offset as u32
    }
}

fn rebase_nested_event_for_rewrite(
    event: StreamingEvent,
    outer_start: u32,
    outer_path: &str,
) -> Option<StreamingEvent> {
    match event {
        StreamingEvent::DocStart(_) | StreamingEvent::DocEnd(_) => None,
        StreamingEvent::MapStart(meta) => Some(StreamingEvent::MapStart(
            rebase_nested_meta_for_rewrite(meta, outer_start, outer_path),
        )),
        StreamingEvent::MapKey { value, meta } => Some(StreamingEvent::MapKey {
            value,
            meta: rebase_nested_meta_for_rewrite(meta, outer_start, outer_path),
        }),
        StreamingEvent::MapEnd(meta) => Some(StreamingEvent::MapEnd(
            rebase_nested_meta_for_rewrite(meta, outer_start, outer_path),
        )),
        StreamingEvent::SeqStart(meta) => Some(StreamingEvent::SeqStart(
            rebase_nested_meta_for_rewrite(meta, outer_start, outer_path),
        )),
        StreamingEvent::SeqEnd(meta) => Some(StreamingEvent::SeqEnd(
            rebase_nested_meta_for_rewrite(meta, outer_start, outer_path),
        )),
        StreamingEvent::Scalar { value, meta } => Some(StreamingEvent::Scalar {
            value,
            meta: rebase_nested_meta_for_rewrite(meta, outer_start, outer_path),
        }),
        StreamingEvent::Alias { anchor, meta } => Some(StreamingEvent::Alias {
            anchor,
            meta: rebase_nested_meta_for_rewrite(meta, outer_start, outer_path),
        }),
        StreamingEvent::ParseError { message, meta } => Some(StreamingEvent::ParseError {
            message,
            meta: rebase_nested_meta_for_rewrite(meta, outer_start, outer_path),
        }),
    }
}

fn rebase_nested_meta_for_rewrite(mut meta: Meta, outer_start: u32, outer_path: &str) -> Meta {
    meta.start_byte = outer_start.saturating_add(meta.start_byte);
    meta.end_byte = outer_start.saturating_add(meta.end_byte);
    meta.path = rebase_nested_path_for_rewrite(&meta.path, outer_path);
    meta.path_supplier = None;
    meta
}

fn rebase_nested_path_for_rewrite(inner_path: &str, outer_path: &str) -> String {
    if outer_path.is_empty() {
        return inner_path.to_string();
    }
    if inner_path.is_empty() || inner_path == "$" {
        return outer_path.to_string();
    }
    let suffix = inner_path.strip_prefix('$').unwrap_or(inner_path);
    format!("{outer_path}{suffix}")
}

fn append_index_path(out: &mut String, index: usize) {
    use std::fmt::Write;
    out.push('[');
    let _ = write!(out, "{index}");
    out.push(']');
}
