use std::collections::HashMap;

use super::config::DEFAULT_SCANNER_COMPACT_THRESHOLD;

// ---------------------------------------------------------------------------
// Position / Span
// ---------------------------------------------------------------------------

/// Tracks the current byte offset, line, and column during scanning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position {
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

/// A half-open `[start, end)` span in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/// Discriminant for every JSON token the scanner can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenTag {
    /// No token available yet – the caller must feed more data.
    None,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Comma,
    String,
    Number,
    TrueKw,
    FalseKw,
    NullKw,
    Comment,
    Eof,
    Invalid,
}

/// A single lexical token produced by the incremental scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub tag: TokenTag,
    pub span: Span,
    /// Owned, decoded value for string / number / keyword tokens.
    pub value: String,
}

impl Token {
    fn simple(tag: TokenTag, start: Position, end: Position) -> Self {
        Self {
            tag,
            span: Span { start, end },
            value: String::new(),
        }
    }

    fn valued(tag: TokenTag, start: Position, end: Position, value: String) -> Self {
        Self {
            tag,
            span: Span { start, end },
            value,
        }
    }
}

// ---------------------------------------------------------------------------
// TokenCache – optional string interning for JSON keys / numbers
// ---------------------------------------------------------------------------

/// Caches decoded string values keyed by their raw source bytes so that
/// repeated occurrences of the same key or number literal share a single
/// allocation.
#[derive(Debug, Clone, Default)]
pub struct TokenCache {
    /// Map from raw JSON bytes (owned) to decoded string value (owned).
    entries: HashMap<Vec<u8>, String>,
}

impl TokenCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a raw byte slice in the cache.
    pub fn get(&self, raw: &[u8]) -> Option<&str> {
        self.entries.get(raw).map(|s| s.as_str())
    }

    /// Insert a mapping from raw bytes to decoded value.
    /// If the key already exists this is a no-op.
    pub fn put(&mut self, raw: &[u8], value: String) {
        if self.entries.contains_key(raw) {
            return;
        }
        self.entries.insert(raw.to_vec(), value);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Scanner – incremental JSON tokeniser
// ---------------------------------------------------------------------------

///
/// Callers push chunks via [`feed`](Scanner::feed) and pull tokens via
/// [`next_token`](Scanner::next_token).  When the buffer contains an
/// incomplete token the scanner returns [`TokenTag::None`]; the caller should
/// feed more data and retry.  When the final chunk has been fed the caller
/// passes `final: true` so that the scanner can emit [`TokenTag::Eof`] or
/// [`TokenTag::Invalid`] instead of stalling.
#[derive(Debug)]
pub struct Scanner {
    /// Accumulated raw bytes that have been fed but not yet consumed.
    buf: Vec<u8>,

    /// Current read position within `buf`.
    cursor: usize,

    /// Snapshot of `cursor` at the start of the current token (for rollback).
    token_start_cursor: usize,

    /// Snapshot of `pos` at the start of the current token (for rollback).
    token_start_pos: Position,

    /// Current source position.
    pos: Position,

    /// Scratch buffer used while decoding a string or number token.
    token_buf: Vec<u8>,

    // ---- string-scanning state ----
    string_escape: bool,
    string_unicode_left: u8,
    string_unicode_value: u32,
    string_unicode_high_surrogate: Option<u16>,

    // ---- token cache ----
    token_cache: Option<TokenCache>,
    token_cache_enabled: bool,

    // ---- buffer management ----
    compact_threshold: usize,
}

impl Clone for Scanner {
    fn clone(&self) -> Self {
        Self {
            buf: self.buf.clone(),
            cursor: self.cursor,
            token_start_cursor: self.token_start_cursor,
            token_start_pos: self.token_start_pos,
            pos: self.pos,
            token_buf: self.token_buf.clone(),
            string_escape: self.string_escape,
            string_unicode_left: self.string_unicode_left,
            string_unicode_value: self.string_unicode_value,
            string_unicode_high_surrogate: self.string_unicode_high_surrogate,
            token_cache: self.token_cache.clone(),
            token_cache_enabled: self.token_cache_enabled,
            compact_threshold: self.compact_threshold,
        }
    }
}

impl Scanner {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Create a new scanner.
    ///
    /// * `token_cache` – if `Some`, the scanner will intern decoded string
    ///   and number values so that repeated occurrences share allocations.
    /// * `token_cache_enabled` – master switch; when `false` the cache is
    ///   never consulted even if a cache instance is provided.
    pub fn new(
        token_cache: Option<TokenCache>,
        token_cache_enabled: bool,
        _unused: Option<()>,
    ) -> Self {
        Self {
            buf: Vec::new(),
            cursor: 0,
            token_start_cursor: 0,
            token_start_pos: Position::default(),
            pos: Position::default(),
            token_buf: Vec::new(),
            string_escape: false,
            string_unicode_left: 0,
            string_unicode_value: 0,
            string_unicode_high_surrogate: None,
            token_cache,
            token_cache_enabled,
            compact_threshold: DEFAULT_SCANNER_COMPACT_THRESHOLD,
        }
    }

    /// Create a scanner with a custom compact threshold.
    pub fn with_compact_threshold(mut self, threshold: usize) -> Self {
        self.compact_threshold = threshold;
        self
    }

    // ------------------------------------------------------------------
    // Feed
    // ------------------------------------------------------------------

    /// Append raw bytes to the internal buffer.
    ///
    /// The chunk does not need to be valid UTF-8 at this point; the scanner
    /// will validate / decode on demand when producing tokens.
    pub fn feed_bytes(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    /// Convenience wrapper that accepts a `&str`.
    pub fn feed(&mut self, chunk: &str) {
        self.feed_bytes(chunk.as_bytes());
    }

    // ------------------------------------------------------------------
    // Token production
    // ------------------------------------------------------------------

    /// Pull the next token from the buffer.
    ///
    /// * `final_chunk` – set to `true` when no more data will be fed.  The
    ///   scanner will then return [`TokenTag::Eof`] at end-of-input or
    ///   [`TokenTag::Invalid`] for incomplete tokens instead of stalling
    ///   with [`TokenTag::None`].
    pub fn next_token(&mut self, final_chunk: bool) -> Token {
        self.next_token_inner(final_chunk)
    }

    // ------------------------------------------------------------------
    // Buffer management
    // ------------------------------------------------------------------

    /// Compact the internal buffer by discarding bytes that have already
    /// been consumed (up to `token_start_cursor`).
    ///
    /// This is called automatically after every complete token, but can
    /// also be invoked explicitly by the caller.
    pub fn compact_buffer(&mut self) {
        if self.token_start_cursor == 0 {
            return;
        }
        let consumed = self.token_start_cursor;
        let remaining = self.buf.len() - consumed;
        if consumed < self.compact_threshold || consumed < remaining {
            return;
        }
        if remaining > 0 {
            self.buf.copy_within(consumed.., 0);
        }
        self.buf.truncate(remaining);
        self.cursor -= consumed;
        self.token_start_cursor = 0;
    }

    /// Return the number of bytes currently buffered but not yet consumed.
    pub fn buffered_len(&self) -> usize {
        self.buf.len().saturating_sub(self.cursor)
    }

    /// Return the total number of bytes fed so far (including consumed).
    pub fn total_fed(&self) -> usize {
        self.pos.offset
    }

    // ------------------------------------------------------------------
    // Accessors
    // ------------------------------------------------------------------

    pub fn position(&self) -> Position {
        self.pos
    }

    pub fn token_cache(&self) -> Option<&TokenCache> {
        self.token_cache.as_ref()
    }

    pub fn token_cache_mut(&mut self) -> Option<&mut TokenCache> {
        self.token_cache.as_mut()
    }

    pub fn token_cache_enabled(&self) -> bool {
        self.token_cache_enabled
    }

    pub fn compact_threshold(&self) -> usize {
        self.compact_threshold
    }

    fn next_token_inner(&mut self, final_chunk: bool) -> Token {
        // Skip whitespace.
        loop {
            let ch = match self.peek_byte() {
                Some(c) => c,
                None => {
                    if final_chunk {
                        return Token::simple(TokenTag::Eof, self.pos, self.pos);
                    }
                    return Token::simple(TokenTag::None, self.pos, self.pos);
                }
            };
            if !is_json_whitespace(ch) {
                break;
            }
            self.advance_byte();
        }

        // Handle comments (non-standard but common in JSON supersets).
        if self.peek_byte() == Some(b'/') {
            if !final_chunk {
                self.mark_token_start();
                self.rollback_token();
                return Token::simple(TokenTag::None, self.pos, self.pos);
            }
            self.mark_token_start();
            let start = self.pos;
            self.advance_byte(); // consume '/'
            if let Some(tok) = self.try_line_comment(start, final_chunk) {
                return tok;
            }
            if let Some(tok) = self.try_block_comment(start, final_chunk) {
                return tok;
            }
            return self.make_invalid_token(start);
        }

        self.mark_token_start();
        let start = self.pos;
        let ch = match self.advance_byte() {
            Some(c) => c,
            None => {
                // peek said there was a byte, so this shouldn't happen.
                return Token::simple(TokenTag::None, self.pos, self.pos);
            }
        };

        match ch {
            b'{' => self.simple_token(TokenTag::LeftBrace, start),
            b'}' => self.simple_token(TokenTag::RightBrace, start),
            b'[' => self.simple_token(TokenTag::LeftBracket, start),
            b']' => self.simple_token(TokenTag::RightBracket, start),
            b':' => self.simple_token(TokenTag::Colon, start),
            b',' => self.simple_token(TokenTag::Comma, start),
            b'"' => self.read_string_token(start, final_chunk),
            b'-' | b'0'..=b'9' => self.read_number_token(start, ch, final_chunk),
            b't' => self.read_literal_token(start, b"true", TokenTag::TrueKw, final_chunk),
            b'f' => self.read_literal_token(start, b"false", TokenTag::FalseKw, final_chunk),
            b'n' => self.read_literal_token(start, b"null", TokenTag::NullKw, final_chunk),
            _ => self.make_invalid_token(start),
        }
    }

    // ------------------------------------------------------------------
    // Simple / literal tokens
    // ------------------------------------------------------------------

    fn simple_token(&mut self, tag: TokenTag, start: Position) -> Token {
        self.compact_buffer();
        Token::simple(tag, start, self.pos)
    }

    fn read_literal_token(
        &mut self,
        start: Position,
        literal: &[u8],
        tag: TokenTag,
        final_chunk: bool,
    ) -> Token {
        // The first byte was already consumed; match the rest.
        for &expected in &literal[1..] {
            let ch = match self.peek_byte() {
                Some(c) => c,
                None => {
                    if !final_chunk {
                        self.rollback_token();
                        return Token::simple(TokenTag::None, start, start);
                    }
                    return self.make_invalid_token(start);
                }
            };
            if ch != expected {
                return self.make_invalid_token(start);
            }
            self.advance_byte();
        }
        self.compact_buffer();
        let value = String::from_utf8_lossy(literal).into_owned();
        Token::valued(tag, start, self.pos, value)
    }

    // ------------------------------------------------------------------
    // String token
    // ------------------------------------------------------------------

    fn read_string_token(&mut self, start: Position, final_chunk: bool) -> Token {
        let raw_start = self.cursor; // position of first content byte after '"'
        self.token_buf.clear();
        self.string_escape = false;
        self.string_unicode_left = 0;
        self.string_unicode_value = 0;
        self.string_unicode_high_surrogate = None;
        let mut had_escape = false;

        loop {
            let ch = match self.advance_byte() {
                Some(c) => c,
                None => {
                    if !final_chunk {
                        self.rollback_token();
                        return Token::simple(TokenTag::None, start, start);
                    }
                    return self.make_invalid_token(start);
                }
            };

            // Mid-unicode escape: accumulate hex digits.
            if self.string_unicode_left != 0 {
                let val = match hex_value(ch) {
                    Some(v) => v,
                    None => return self.make_invalid_token(start),
                };
                self.string_unicode_value = (self.string_unicode_value << 4) | val as u32;
                self.string_unicode_left -= 1;
                if self.string_unicode_left == 0 {
                    if !self.append_decoded_unicode_scalar(self.string_unicode_value as u16) {
                        return self.make_invalid_token(start);
                    }
                }
                continue;
            }

            // Immediately after a backslash.
            if self.string_escape {
                had_escape = true;
                self.string_escape = false;
                match ch {
                    b'"' => self.token_buf.push(b'"'),
                    b'\\' => self.token_buf.push(b'\\'),
                    b'/' => self.token_buf.push(b'/'),
                    b'b' => self.token_buf.push(0x08),
                    b'f' => self.token_buf.push(0x0C),
                    b'n' => self.token_buf.push(b'\n'),
                    b'r' => self.token_buf.push(b'\r'),
                    b't' => self.token_buf.push(b'\t'),
                    b'u' => {
                        self.string_unicode_left = 4;
                        self.string_unicode_value = 0;
                    }
                    _ => return self.make_invalid_token(start),
                }
                continue;
            }

            // Start of escape sequence.
            if ch == b'\\' {
                had_escape = true;
                self.string_escape = true;
                continue;
            }

            // Closing quote.
            if ch == b'"' {
                self.flush_pending_high_surrogate();
                if !is_valid_utf8(&self.token_buf) {
                    return self.make_invalid_token(start);
                }
                let out = self.finish_string_token(raw_start, had_escape);
                self.compact_buffer();
                return Token::valued(TokenTag::String, start, self.pos, out);
            }

            // Control characters (codepoints < 0x20) are illegal in JSON strings.
            if ch < 0x20 {
                return self.make_invalid_token(start);
            }

            self.token_buf.push(ch);
        }
    }

    fn finish_string_token(&mut self, raw_start: usize, had_escape: bool) -> String {
        let result = if self.token_cache_enabled {
            if let Some(ref cache) = self.token_cache {
                if !had_escape && self.cursor > 0 && self.cursor > raw_start + 1 {
                    let raw_end = self.cursor - 1; // exclude closing '"'
                    if raw_end > raw_start && raw_end <= self.buf.len() {
                        let raw = &self.buf[raw_start..raw_end];
                        if !raw.is_empty() {
                            if let Some(cached) = cache.get(raw) {
                                return cached.to_string();
                            }
                        }
                    }
                }
            }
            // Fall through: allocate a fresh string.
            self.dup_token_buffer()
        } else {
            self.dup_token_buffer()
        };

        // Store in cache if enabled.
        if self.token_cache_enabled {
            if let Some(ref mut cache) = self.token_cache {
                if !had_escape && self.cursor > 0 && self.cursor > raw_start + 1 {
                    let raw_end = self.cursor - 1;
                    if raw_end > raw_start && raw_end <= self.buf.len() {
                        let raw = &self.buf[raw_start..raw_end];
                        if !raw.is_empty() {
                            cache.put(raw, result.clone());
                        }
                    }
                }
            }
        }

        result
    }

    // ------------------------------------------------------------------
    // Number token
    // ------------------------------------------------------------------

    fn read_number_token(&mut self, start: Position, first: u8, final_chunk: bool) -> Token {
        let raw_start = self.cursor - 1; // include the first byte we already consumed
        self.token_buf.clear();
        self.token_buf.push(first);

        loop {
            let ch = match self.peek_byte() {
                Some(c) => c,
                None => break,
            };
            if ch.is_ascii_digit() {
                self.advance_byte();
                self.token_buf.push(ch);
                continue;
            }
            if matches!(ch, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.advance_byte();
                self.token_buf.push(ch);
                continue;
            }
            break;
        }

        // If we're at end of buffer and not final, stall.
        if self.peek_byte().is_none() && !final_chunk {
            self.rollback_token();
            return Token::simple(TokenTag::None, start, start);
        }

        if !is_valid_json_number(&self.token_buf) {
            return self.make_invalid_token(start);
        }

        let out = self.finish_number_token(raw_start, self.cursor);
        self.compact_buffer();
        Token::valued(TokenTag::Number, start, self.pos, out)
    }

    fn finish_number_token(&mut self, raw_start: usize, raw_end: usize) -> String {
        let result = if self.token_cache_enabled {
            if let Some(ref cache) = self.token_cache {
                if raw_end > raw_start && raw_end <= self.buf.len() {
                    let raw = &self.buf[raw_start..raw_end];
                    if !raw.is_empty() {
                        if let Some(cached) = cache.get(raw) {
                            return cached.to_string();
                        }
                    }
                }
            }
            self.dup_token_buffer()
        } else {
            self.dup_token_buffer()
        };

        // Store in cache.
        if self.token_cache_enabled {
            if let Some(ref mut cache) = self.token_cache {
                if raw_end > raw_start && raw_end <= self.buf.len() {
                    let raw = &self.buf[raw_start..raw_end];
                    if !raw.is_empty() {
                        cache.put(raw, result.clone());
                    }
                }
            }
        }

        result
    }

    // ------------------------------------------------------------------
    // Unicode helpers
    // ------------------------------------------------------------------

    /// Decode a single UTF-16 code unit (from `\uXXXX`) and append the
    /// corresponding UTF-8 bytes to `token_buf`.  Handles surrogate pairs.
    ///
    /// Returns `false` when the scalar is invalid.
    fn append_decoded_unicode_scalar(&mut self, value: u16) -> bool {
        // If we have a pending high surrogate, try to complete the pair.
        if let Some(high) = self.string_unicode_high_surrogate.take() {
            if (0xDC00..=0xDFFF).contains(&value) {
                // Valid low surrogate – combine into a supplementary scalar.
                let high_ten = (high as u32) - 0xD800;
                let low_ten = (value as u32) - 0xDC00;
                let scalar = 0x10000 + ((high_ten << 10) | low_ten);
                self.append_unicode_scalar(scalar);
                return true;
            }
            // Invalid: high surrogate not followed by low surrogate.
            // Emit the high surrogate as a lone code unit and fall through.
            self.append_unicode_code_unit(high);
        }

        if (0xD800..=0xDBFF).contains(&value) {
            // High surrogate – save for the next escape.
            self.string_unicode_high_surrogate = Some(value);
            return true;
        }

        if (0xDC00..=0xDFFF).contains(&value) {
            // Lone low surrogate – encode as-is (non-standard but tolerated).
            self.append_unicode_code_unit(value);
            return true;
        }

        self.append_unicode_scalar(value as u32);
        true
    }

    fn flush_pending_high_surrogate(&mut self) {
        if let Some(high) = self.string_unicode_high_surrogate.take() {
            self.append_unicode_code_unit(high);
        }
    }

    fn append_unicode_scalar(&mut self, scalar: u32) {
        if let Some(c) = char::from_u32(scalar) {
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            self.token_buf.extend_from_slice(encoded.as_bytes());
        } else {
            // Invalid scalar – emit replacement character.
            self.token_buf.extend_from_slice("\u{FFFD}".as_bytes());
        }
    }

    fn append_unicode_code_unit(&mut self, value: u16) {
        // Encode a single UTF-16 code unit as UTF-8 (BMP only).
        match value {
            0..=0x7F => {
                self.token_buf.push(value as u8);
            }
            0x80..=0x7FF => {
                self.token_buf.push(0xC0 | (value >> 6) as u8);
                self.token_buf.push(0x80 | (value & 0x3F) as u8);
            }
            _ => {
                self.token_buf.push(0xE0 | (value >> 12) as u8);
                self.token_buf.push(0x80 | ((value >> 6) & 0x3F) as u8);
                self.token_buf.push(0x80 | (value & 0x3F) as u8);
            }
        }
    }

    // ------------------------------------------------------------------
    // Comment helpers
    // ------------------------------------------------------------------

    fn try_line_comment(&mut self, start: Position, final_chunk: bool) -> Option<Token> {
        let next = self.peek_byte()?;
        if next != b'/' {
            return None;
        }
        self.advance_byte(); // consume second '/'

        loop {
            let ch = match self.peek_byte() {
                Some(c) => c,
                None => {
                    if !final_chunk {
                        self.rollback_token();
                        return None;
                    }
                    break;
                }
            };
            if ch == b'\n' {
                break;
            }
            self.advance_byte();
        }

        self.compact_buffer();
        Some(Token::simple(TokenTag::Comment, start, self.pos))
    }

    fn try_block_comment(&mut self, start: Position, final_chunk: bool) -> Option<Token> {
        let next = self.peek_byte()?;
        if next != b'*' {
            return None;
        }
        self.advance_byte(); // consume '*'

        let mut prev: u8 = 0;
        loop {
            let ch = match self.peek_byte() {
                Some(c) => c,
                None => {
                    if !final_chunk {
                        self.rollback_token();
                        return None;
                    }
                    break;
                }
            };
            self.advance_byte();
            if prev == b'*' && ch == b'/' {
                break;
            }
            prev = ch;
        }

        self.compact_buffer();
        Some(Token::simple(TokenTag::Comment, start, self.pos))
    }

    // ------------------------------------------------------------------
    // Low-level cursor helpers
    // ------------------------------------------------------------------

    fn mark_token_start(&mut self) {
        self.token_start_cursor = self.cursor;
        self.token_start_pos = self.pos;
    }

    fn rollback_token(&mut self) {
        self.cursor = self.token_start_cursor;
        self.pos = self.token_start_pos;
    }

    fn make_invalid_token(&mut self, start: Position) -> Token {
        let span = Span {
            start,
            end: self.pos,
        };
        self.compact_buffer();
        Token {
            tag: TokenTag::Invalid,
            span,
            value: String::new(),
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.buf.get(self.cursor).copied()
    }

    fn advance_byte(&mut self) -> Option<u8> {
        let ch = self.peek_byte()?;
        self.cursor += 1;
        self.pos.offset += 1;
        if ch == b'\n' {
            self.pos.line += 1;
            self.pos.column = 0;
        } else {
            self.pos.column += 1;
        }
        Some(ch)
    }

    fn dup_token_buffer(&self) -> String {
        String::from_utf8_lossy(&self.token_buf).into_owned()
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

fn is_json_whitespace(ch: u8) -> bool {
    matches!(ch, b' ' | b'\t' | b'\n' | b'\r')
}

fn hex_value(ch: u8) -> Option<u8> {
    match ch {
        b'0'..=b'9' => Some(ch - b'0'),
        b'a'..=b'f' => Some(ch - b'a' + 10),
        b'A'..=b'F' => Some(ch - b'A' + 10),
        _ => None,
    }
}

fn is_valid_json_number(raw: &[u8]) -> bool {
    if raw.is_empty() {
        return false;
    }

    let mut i = 0usize;

    // Optional leading minus.
    if raw[i] == b'-' {
        i += 1;
        if i == raw.len() {
            return false;
        }
    }

    // Integer part.
    match raw[i] {
        b'0' => {
            i += 1;
        }
        b'1'..=b'9' => {
            i += 1;
            while i < raw.len() && raw[i].is_ascii_digit() {
                i += 1;
            }
        }
        _ => return false,
    }

    // Optional fraction.
    if i < raw.len() && raw[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < raw.len() && raw[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return false;
        }
    }

    // Optional exponent.
    if i < raw.len() && (raw[i] == b'e' || raw[i] == b'E') {
        i += 1;
        if i < raw.len() && (raw[i] == b'+' || raw[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < raw.len() && raw[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }

    i == raw.len()
}

fn is_valid_utf8(buf: &[u8]) -> bool {
    std::str::from_utf8(buf).is_ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Basic token recognition
    // ------------------------------------------------------------------

    #[test]
    fn scans_simple_object() {
        let mut s = Scanner::new(None, false, None);
        s.feed("{}");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::LeftBrace);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::RightBrace);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Eof);
    }

    #[test]
    fn scans_punctuation() {
        let mut s = Scanner::new(None, false, None);
        s.feed("[,]:");
        assert_eq!(s.next_token(true).tag, TokenTag::LeftBracket);
        assert_eq!(s.next_token(true).tag, TokenTag::Comma);
        assert_eq!(s.next_token(true).tag, TokenTag::RightBracket);
        assert_eq!(s.next_token(true).tag, TokenTag::Colon);
        assert_eq!(s.next_token(true).tag, TokenTag::Eof);
    }

    #[test]
    fn scans_keywords() {
        let mut s = Scanner::new(None, false, None);
        s.feed("true false null");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::TrueKw);
        assert_eq!(tok.value, "true");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::FalseKw);
        assert_eq!(tok.value, "false");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::NullKw);
        assert_eq!(tok.value, "null");
    }

    #[test]
    fn scans_string() {
        let mut s = Scanner::new(None, false, None);
        s.feed(r#""hello""#);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::String);
        assert_eq!(tok.value, "hello");
    }

    #[test]
    fn scans_string_with_escapes() {
        let mut s = Scanner::new(None, false, None);
        s.feed(r#""a\nb\tc\\d\"e""#);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::String);
        assert_eq!(tok.value, "a\nb\tc\\d\"e");
    }

    #[test]
    fn scans_number_integer() {
        let mut s = Scanner::new(None, false, None);
        s.feed("42");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Number);
        assert_eq!(tok.value, "42");
    }

    #[test]
    fn scans_number_negative() {
        let mut s = Scanner::new(None, false, None);
        s.feed("-123");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Number);
        assert_eq!(tok.value, "-123");
    }

    #[test]
    fn scans_number_float() {
        let mut s = Scanner::new(None, false, None);
        s.feed("3.14");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Number);
        assert_eq!(tok.value, "3.14");
    }

    #[test]
    fn scans_number_exponent() {
        let mut s = Scanner::new(None, false, None);
        s.feed("1e10");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Number);
        assert_eq!(tok.value, "1e10");
    }

    // ------------------------------------------------------------------
    // Incremental / feed behaviour
    // ------------------------------------------------------------------

    #[test]
    fn stalls_on_partial_input() {
        let mut s = Scanner::new(None, false, None);
        s.feed("\"hel");
        let tok = s.next_token(false);
        assert_eq!(tok.tag, TokenTag::None);
        // Feed the rest.
        s.feed("lo\"");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::String);
        assert_eq!(tok.value, "hello");
    }

    #[test]
    fn stalls_on_partial_keyword() {
        let mut s = Scanner::new(None, false, None);
        s.feed("tru");
        let tok = s.next_token(false);
        assert_eq!(tok.tag, TokenTag::None);
        s.feed("e");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::TrueKw);
    }

    #[test]
    fn stalls_on_partial_number() {
        let mut s = Scanner::new(None, false, None);
        s.feed("12.");
        let tok = s.next_token(false);
        assert_eq!(tok.tag, TokenTag::None);
        s.feed("5");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Number);
        assert_eq!(tok.value, "12.5");
    }

    #[test]
    fn final_chunk_produces_eof() {
        let mut s = Scanner::new(None, false, None);
        s.feed("1");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Number);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Eof);
    }

    #[test]
    fn final_chunk_produces_invalid_for_unterminated_string() {
        let mut s = Scanner::new(None, false, None);
        s.feed("\"oops");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Invalid);
    }

    // ------------------------------------------------------------------
    // Unicode / surrogate pairs
    // ------------------------------------------------------------------

    #[test]
    fn decodes_bmp_unicode_escape() {
        let mut s = Scanner::new(None, false, None);
        s.feed(r#""\u4f60\u597d""#); // 你好
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::String);
        assert_eq!(tok.value, "你好");
    }

    #[test]
    fn decodes_surrogate_pair() {
        let mut s = Scanner::new(None, false, None);
        // U+1F600 (grinning face) = surrogate pair D83D DE00
        s.feed(r#""\uD83D\uDE00""#);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::String);
        assert_eq!(tok.value, "\u{1F600}");
    }

    #[test]
    fn handles_lone_high_surrogate() {
        let mut s = Scanner::new(None, false, None);
        s.feed(r#""\uD83D""#);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Invalid);
    }

    #[test]
    fn handles_lone_low_surrogate() {
        let mut s = Scanner::new(None, false, None);
        s.feed(r#""\uDE00""#);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Invalid);
    }

    // ------------------------------------------------------------------
    // Control character diagnostics
    // ------------------------------------------------------------------

    #[test]
    fn rejects_control_char_in_string() {
        let mut s = Scanner::new(None, false, None);
        // Tab is 0x09, which is < 0x20.
        let input = vec![b'"', 0x09, b'"'];
        s.feed_bytes(&input);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Invalid);
    }

    // ------------------------------------------------------------------
    // Comments (non-standard JSON)
    // ------------------------------------------------------------------

    #[test]
    fn scans_line_comment() {
        let mut s = Scanner::new(None, false, None);
        s.feed("// hello\n1");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Comment);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Number);
    }

    #[test]
    fn scans_block_comment() {
        let mut s = Scanner::new(None, false, None);
        s.feed("/* comment */1");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Comment);
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::Number);
    }

    // ------------------------------------------------------------------
    // Token cache
    // ------------------------------------------------------------------

    #[test]
    fn token_cache_dedup_strings() {
        let cache = TokenCache::new();
        let mut s = Scanner::new(Some(cache), true, None);
        s.feed(r#""hello" "hello""#);
        let tok1 = s.next_token(true);
        assert_eq!(tok1.tag, TokenTag::String);
        let tok2 = s.next_token(true);
        assert_eq!(tok2.tag, TokenTag::String);
        // Both values should be equal.
        assert_eq!(tok1.value, tok2.value);
        // The cache should contain one entry.
        assert_eq!(s.token_cache().unwrap().entries.len(), 1);
    }

    #[test]
    fn token_cache_disabled() {
        let cache = TokenCache::new();
        let mut s = Scanner::new(Some(cache), false, None);
        s.feed(r#""hello" "hello""#);
        let _tok1 = s.next_token(true);
        let _tok2 = s.next_token(true);
        // Cache should still be empty because it was disabled.
        assert!(s.token_cache().unwrap().is_empty());
    }

    // ------------------------------------------------------------------
    // Buffer compaction
    // ------------------------------------------------------------------

    #[test]
    fn compacts_buffer_after_tokens() {
        let mut s = Scanner::new(None, false, None).with_compact_threshold(4); // low threshold to force compaction
        // Feed enough tokens to trigger compaction.
        s.feed("1 2 3 4 5");
        for _ in 0..5 {
            let tok = s.next_token(true);
            assert_eq!(tok.tag, TokenTag::Number);
        }
        // Buffer should be compact (cursor near 0).
        assert!(s.buffered_len() <= 5);
    }

    // ------------------------------------------------------------------
    // Position tracking
    // ------------------------------------------------------------------

    #[test]
    fn tracks_line_and_column() {
        let mut s = Scanner::new(None, false, None);
        s.feed("{\n  \"key\" : 1\n}");
        let tok = s.next_token(true);
        assert_eq!(tok.tag, TokenTag::LeftBrace);
        assert_eq!(tok.span.start.line, 0);
        assert_eq!(tok.span.start.column, 0);

        // Skip to the string token.
        let tok = s.next_token(true); // string "key"
        assert_eq!(tok.tag, TokenTag::String);
        assert_eq!(tok.span.start.line, 1);
    }
}
