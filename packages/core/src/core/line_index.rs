use super::incremental_edit::DocumentTextEdit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineColumn {
    pub line: u32,
    pub column: u32,
}

/// Start and end byte offsets of a line.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineBounds {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    source_len: usize,
}

impl Default for LineIndex {
    fn default() -> Self {
        Self {
            line_starts: vec![0],
            source_len: 0,
        }
    }
}

impl LineIndex {
    /// Build from a full-text scan (existing path).
    pub fn build(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut line_starts =
            Vec::with_capacity(bytes.iter().filter(|byte| **byte == b'\n').count() + 1);
        line_starts.push(0);
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'\n' && index + 1 <= bytes.len() {
                line_starts.push(index + 1);
            }
        }
        Self {
            line_starts,
            source_len: bytes.len(),
        }
    }

    /// Derive the next index from this index and one edit expressed against the
    /// previous source. This scans only the replacement bytes plus stored line
    /// starts; it does not rescan the full next source text.
    pub fn apply_single_edit(&self, edit: &DocumentTextEdit) -> Option<Self> {
        let start = edit.start_byte as usize;
        let old_end = edit.old_end_byte as usize;
        let new_end = edit.new_end_byte as usize;
        if edit_byte_range_len(self.source_len, edit).is_none() {
            return None;
        }
        if new_end != start.checked_add(edit.replacement.len())? {
            return None;
        }

        let delta = new_end as isize - old_end as isize;
        let mut line_starts = Vec::with_capacity(
            self.line_starts.len()
                + edit
                    .replacement
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count(),
        );

        for offset in self
            .line_starts
            .iter()
            .copied()
            .take_while(|offset| *offset <= start)
        {
            push_unique_line_start(&mut line_starts, offset);
        }

        for (index, byte) in edit.replacement.bytes().enumerate() {
            if byte == b'\n' {
                push_unique_line_start(&mut line_starts, start + index + 1);
            }
        }

        for offset in self
            .line_starts
            .iter()
            .copied()
            .filter(|offset| *offset >= old_end)
        {
            if old_end == start && offset == start {
                continue;
            }
            let shifted = apply_line_start_delta(offset, delta)?;
            push_unique_line_start(&mut line_starts, shifted);
        }

        if line_starts.is_empty() || line_starts[0] != 0 {
            line_starts.insert(0, 0);
        }
        let source_len = apply_line_start_delta(self.source_len, delta)?;
        line_starts.sort_unstable();
        line_starts.dedup();
        Some(Self {
            line_starts,
            source_len,
        })
    }

    /// Build from pre-computed newline byte offsets (streaming close fast path).
    /// `newline_offsets` must be sorted ascending, each giving the byte offset
    /// of a `\n` character in the source text.
    pub fn from_line_starts(mut line_starts: Vec<usize>) -> Self {
        // Ensure line 0 starts at offset 0.
        if line_starts.first() != Some(&0) {
            line_starts.insert(0, 0);
        }
        let source_len = line_starts.last().copied().unwrap_or(0);
        Self {
            line_starts,
            source_len,
        }
    }

    /// Build from pre-computed line starts and the final source length.
    pub fn from_line_starts_and_len(mut line_starts: Vec<usize>, source_len: usize) -> Self {
        if line_starts.first() != Some(&0) {
            line_starts.insert(0, 0);
        }
        Self {
            line_starts,
            source_len,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn source_len(&self) -> usize {
        self.source_len
    }

    pub fn line_start(&self, line: u32) -> Option<usize> {
        self.line_starts.get(line as usize).copied()
    }

    pub fn line_end(&self, line: u32) -> Option<usize> {
        let line = line as usize;
        let start = *self.line_starts.get(line)?;
        let end = self
            .line_starts
            .get(line + 1)
            .map(|offset| offset.saturating_sub(1))
            .unwrap_or(self.source_len);
        Some(end.max(start))
    }

    pub fn offset_to_line_column(&self, offset: usize) -> LineColumn {
        let clamped = offset.min(self.source_len);
        let line_index = match self.line_starts.binary_search(&clamped) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        LineColumn {
            line: line_index as u32,
            column: (clamped - line_start) as u32,
        }
    }

    pub fn line_column_to_offset(&self, line: u32, column: u32) -> usize {
        let Some(line_start) = self.line_start(line) else {
            return self.source_len;
        };
        let line_end = self.line_end(line).unwrap_or(self.source_len);
        line_start.saturating_add(column as usize).min(line_end)
    }

    /// Return the byte-range [`LineBounds`] for a given 0-based row.
    ///
    /// Returns `None` when `row` is out of bounds.
    ///
    pub fn line_bounds(&self, source_len: usize, row: u32) -> Option<LineBounds> {
        let row_index = row as usize;
        if self.line_starts.is_empty() || row_index >= self.line_starts.len() {
            return None;
        }
        let line_start = self.line_starts[row_index];
        let line_end = if row_index + 1 < self.line_starts.len() {
            self.line_starts[row_index + 1].saturating_sub(1)
        } else {
            source_len
        };
        Some(LineBounds {
            start: line_start,
            end: line_end,
        })
    }
}

fn edit_byte_range_len(source_len: usize, edit: &DocumentTextEdit) -> Option<(usize, usize)> {
    let start = edit.start_byte as usize;
    let old_end = edit.old_end_byte as usize;
    if start > old_end || old_end > source_len {
        return None;
    }
    Some((start, old_end))
}

fn apply_line_start_delta(offset: usize, delta: isize) -> Option<usize> {
    if delta >= 0 {
        offset.checked_add(delta as usize)
    } else {
        offset.checked_sub((-delta) as usize)
    }
}

fn push_unique_line_start(line_starts: &mut Vec<usize>, offset: usize) {
    if line_starts.last().copied() != Some(offset) {
        line_starts.push(offset);
    }
}

#[cfg(test)]
mod tests {
    use super::{LineBounds, LineColumn, LineIndex};

    #[test]
    fn line_index_tracks_line_boundaries_and_offsets() {
        let index = LineIndex::build("ab\ncd\nef");
        assert_eq!(index.line_count(), 3);
        assert_eq!(index.line_start(1), Some(3));
        assert_eq!(index.line_end(1), Some(5));
        assert_eq!(
            index.offset_to_line_column(4),
            LineColumn { line: 1, column: 1 }
        );
        assert_eq!(index.line_column_to_offset(2, 1), 7);
    }

    #[test]
    fn line_index_clamps_offsets_past_source_end() {
        let index = LineIndex::build("abc");
        assert_eq!(
            index.offset_to_line_column(99),
            LineColumn { line: 0, column: 3 }
        );
        assert_eq!(index.line_column_to_offset(9, 9), 3);
    }

    #[test]
    fn line_bounds_handles_middle_and_final_lines() {
        let source = "ab\ncd\nef";
        let index = LineIndex::build(source);

        assert_eq!(
            index.line_bounds(source.len(), 1),
            Some(LineBounds { start: 3, end: 5 })
        );
        assert_eq!(
            index.line_bounds(source.len(), 2),
            Some(LineBounds { start: 6, end: 8 })
        );
        assert_eq!(index.line_bounds(source.len(), 3), None);
    }

    #[test]
    fn line_bounds_handles_first_empty_and_trailing_lines() {
        let empty = "";
        let empty_index = LineIndex::build(empty);
        assert_eq!(
            empty_index.line_bounds(empty.len(), 0),
            Some(LineBounds { start: 0, end: 0 })
        );

        let source = "a\n";
        let index = LineIndex::build(source);
        assert_eq!(index.line_count(), 2);
        assert_eq!(
            index.line_bounds(source.len(), 0),
            Some(LineBounds { start: 0, end: 1 })
        );
        assert_eq!(
            index.line_bounds(source.len(), 1),
            Some(LineBounds { start: 2, end: 2 })
        );
    }

    #[test]
    fn offset_and_line_column_round_trip_across_blank_lines() {
        let source = "a\nbc\n\ndef";
        let index = LineIndex::build(source);

        for offset in [0, 1, 2, 5, source.len(), source.len() + 10] {
            let position = index.offset_to_line_column(offset);
            assert_eq!(
                index.line_column_to_offset(position.line, position.column),
                offset.min(source.len())
            );
        }
    }

    #[test]
    fn line_column_to_offset_clamps_columns_and_missing_rows() {
        let source = "a\nbc\n";
        let index = LineIndex::build(source);

        assert_eq!(index.line_column_to_offset(0, 0), 0);
        assert_eq!(index.line_column_to_offset(1, 0), 2);
        assert_eq!(index.line_column_to_offset(1, 99), 4);
        assert_eq!(index.line_column_to_offset(2, 0), source.len());
        assert_eq!(index.line_column_to_offset(3, 0), source.len());
    }
    use crate::core::incremental_edit::DocumentTextEdit;

    #[test]
    fn line_index_apply_single_edit_replaces_middle_line() {
        let source = "root:\n  a: 1\n  b: 2\n";
        let start = source.find("a: 1").expect("fixture contains old line") as u32;
        let replacement = "alpha: 10";
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + "a: 1".len() as u32,
            new_end_byte: start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        };
        let updated_source = source.replacen("a: 1", replacement, 1);

        let base = LineIndex::build(source);
        let updated = base
            .apply_single_edit(&edit)
            .expect("valid single edit should produce a next line index");

        assert_eq!(updated, LineIndex::build(&updated_source));
        assert_eq!(
            updated.offset_to_line_column(updated_source.find("b: 2").unwrap()),
            LineColumn { line: 2, column: 2 }
        );
    }

    #[test]
    fn line_index_apply_single_edit_inserts_newline_at_line_start() {
        let source = "a\nb";
        let start = source.find('b').expect("fixture contains second line") as u32;
        let replacement = "x\n";
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start,
            new_end_byte: start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        };
        let updated_source = "a\nx\nb";

        let base = LineIndex::build(source);
        let updated = base
            .apply_single_edit(&edit)
            .expect("valid insertion should produce a next line index");

        assert_eq!(updated, LineIndex::build(updated_source));
        assert_eq!(updated.line_count(), 3);
        assert_eq!(updated.line_start(2), Some(4));
    }

    #[test]
    fn line_index_apply_single_edit_inserts_without_newline_at_line_start() {
        let source = "a\nb";
        let start = source.find('b').expect("fixture contains second line") as u32;
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start,
            new_end_byte: start + 1,
            replacement: "x".to_owned(),
        };
        let updated_source = "a\nxb";

        let base = LineIndex::build(source);
        let updated = base
            .apply_single_edit(&edit)
            .expect("valid insertion should produce a next line index");

        assert_eq!(updated, LineIndex::build(updated_source));
        assert_eq!(updated.line_start(1), Some(2));
    }

    #[test]
    fn line_index_apply_single_edit_deletes_prefix_line() {
        let source = "gone\nkept\n";
        let edit = DocumentTextEdit {
            start_byte: 0,
            old_end_byte: "gone\n".len() as u32,
            new_end_byte: 0,
            replacement: String::new(),
        };
        let updated_source = "kept\n";

        let base = LineIndex::build(source);
        let updated = base
            .apply_single_edit(&edit)
            .expect("valid deletion should produce a next line index");

        assert_eq!(updated, LineIndex::build(updated_source));
        assert_eq!(updated.line_start(0), Some(0));
        assert_eq!(updated.line_start(1), Some(5));
    }

    #[test]
    fn line_index_apply_single_edit_rejects_invalid_ranges() {
        let source = "abc";
        let base = LineIndex::build(source);
        let edit = DocumentTextEdit {
            start_byte: 3,
            old_end_byte: 1,
            new_end_byte: 3,
            replacement: String::new(),
        };

        assert!(base.apply_single_edit(&edit).is_none());
    }
}
