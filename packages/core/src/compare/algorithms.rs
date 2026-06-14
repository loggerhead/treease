use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffType {
    Insert,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diff {
    pub offset: i32,
    pub length: i32,
    pub diff_type: DiffType,
    pub inline_diffs: Vec<Diff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffPair {
    pub left: Option<Diff>,
    pub right: Option<Diff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassifiedDiffs {
    pub left: Vec<Diff>,
    pub right: Vec<Diff>,
}

pub const MAX_EDIT_LENGTH: usize = 1000;
const INLINE_MAX_EDIT_LENGTH: usize = 100;

#[derive(Debug, Clone, Copy)]
pub struct DiffOptions {
    pub is_array_diff: bool,
    pub max_edit_length: usize,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            is_array_diff: false,
            max_edit_length: MAX_EDIT_LENGTH,
        }
    }
}

#[derive(Debug, Clone)]
struct ComponentValue {
    count: usize,
    value: String,
    added: bool,
    removed: bool,
}

#[derive(Debug, Clone, Copy)]
struct ComponentNode {
    count: usize,
    added: bool,
    removed: bool,
    previous: Option<usize>,
}

#[derive(Debug, Clone)]
struct PathState {
    new_pos: i32,
    last_component: Option<usize>,
}

#[derive(Debug, Clone)]
struct IndexList {
    items: Vec<usize>,
    start: usize,
}

impl IndexList {
    fn append(&mut self, index: usize) {
        self.items.push(index);
    }

    fn slice(&self) -> &[usize] {
        if self.start >= self.items.len() {
            &[]
        } else {
            &self.items[self.start..]
        }
    }

    fn first(&self) -> Option<usize> {
        self.slice().first().copied()
    }

    fn pop_first(&mut self) {
        if self.start < self.items.len() {
            self.start += 1;
        }
    }

    fn len(&self) -> usize {
        self.items.len().saturating_sub(self.start)
    }
}

#[derive(Debug)]
struct Histogram<'a> {
    lines: HashMap<&'a str, IndexList>,
}

impl<'a> Histogram<'a> {
    fn from_lines(array: &[&'a str], start: usize, end: usize) -> Self {
        let mut lines = HashMap::new();
        for (idx, line) in array[start..end].iter().enumerate() {
            let key = trim_line(line);
            lines
                .entry(key)
                .or_insert_with(|| IndexList {
                    items: Vec::new(),
                    start: 0,
                })
                .append(start + idx);
        }
        Self { lines }
    }

    fn get(&self, line: &str) -> &[usize] {
        self.lines
            .get(trim_line(line))
            .map(IndexList::slice)
            .unwrap_or(&[])
    }

    fn num(&self, line: &str) -> usize {
        self.lines
            .get(trim_line(line))
            .map(IndexList::len)
            .unwrap_or(0)
    }

    fn first(&self, line: &str) -> Option<usize> {
        self.lines.get(trim_line(line)).and_then(IndexList::first)
    }

    fn del_first(&mut self, line: &str) {
        if let Some(list) = self.lines.get_mut(trim_line(line)) {
            list.pop_first();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MatchRegion {
    a_start: usize,
    a_end: usize,
    b_start: usize,
    b_end: usize,
    match_score: usize,
}

impl MatchRegion {
    fn valid_start(self, a_start: usize, b_start: usize) -> bool {
        a_start < self.a_start && b_start < self.b_start
    }

    fn valid_end(self, a_end: usize, b_end: usize) -> bool {
        self.a_end < a_end && self.b_end < b_end
    }

    fn length(self) -> usize {
        self.a_end - self.a_start
    }
}

struct HistogramDiffer<'a> {
    aa: Vec<&'a str>,
    bb: Vec<&'a str>,
}

impl<'a> HistogramDiffer<'a> {
    fn new(aa: Vec<&'a str>, bb: Vec<&'a str>) -> Self {
        Self { aa, bb }
    }

    fn eq(&self, a_idx: usize, b_idx: usize) -> bool {
        trim_line(self.aa[a_idx]) == trim_line(self.bb[b_idx])
    }

    fn longest_substring(
        &self,
        a_start: usize,
        a_end: usize,
        b_start: usize,
        b_end: usize,
    ) -> Option<MatchRegion> {
        let histogram = Histogram::from_lines(&self.aa, a_start, a_end);
        let mut best_match = None;
        let mut best_match_score = a_end - a_start;
        let mut b_idx = b_start;

        while b_idx < b_end {
            let mut next_b = b_idx + 1;
            let line_b = trim_line(self.bb[b_idx]);
            if histogram.num(line_b) > best_match_score {
                b_idx = next_b;
                continue;
            }

            let mut prev_a = a_start;
            for &a_idx in histogram.get(line_b) {
                if a_idx < prev_a {
                    continue;
                }

                let mut region = MatchRegion {
                    a_start: a_idx,
                    a_end: a_idx + 1,
                    b_start: b_idx,
                    b_end: b_idx + 1,
                    match_score: a_end - a_start,
                };

                while region.valid_start(a_start, b_start)
                    && self.eq(region.a_start - 1, region.b_start - 1)
                {
                    region.a_start -= 1;
                    region.b_start -= 1;
                    if region.match_score > 1 {
                        region.match_score = region
                            .match_score
                            .min(histogram.num(self.aa[region.a_start]));
                    }
                }

                while region.valid_end(a_end, b_end) && self.eq(region.a_end, region.b_end) {
                    if region.match_score > 1 {
                        region.match_score =
                            region.match_score.min(histogram.num(self.aa[region.a_end]));
                    }
                    region.a_end += 1;
                    region.b_end += 1;
                }

                if best_match.is_some_and(|current: MatchRegion| current.length() < region.length())
                    || region.match_score < best_match_score
                {
                    best_match = Some(region);
                    best_match_score = region.match_score;
                }

                if next_b < region.b_end {
                    next_b = region.b_end;
                }
                prev_a = region.a_end;
            }

            b_idx = next_b;
        }

        best_match
    }

    fn solve_range(
        &self,
        a_start: usize,
        a_end: usize,
        b_start: usize,
        b_end: usize,
        out: &mut Vec<MatchRegion>,
    ) {
        if b_end - b_start <= 1 || a_end - a_start <= 1 {
            return;
        }

        let Some(region) = self.longest_substring(a_start, a_end, b_start, b_end) else {
            return;
        };

        self.solve_range(a_start, region.a_start, b_start, region.b_start, out);
        out.push(region);
        self.solve_range(region.a_end, a_end, region.b_end, b_end, out);
    }

    fn find_common_lines(&self, aa: &[&str], bb: &[&str], out: &mut Vec<MatchRegion>) {
        let mut histogram = Histogram::from_lines(aa, 0, aa.len());
        for (b_idx, line) in bb.iter().enumerate() {
            if let Some(a_idx) = histogram.first(line) {
                out.push(MatchRegion {
                    a_start: a_idx,
                    a_end: a_idx + 1,
                    b_start: b_idx,
                    b_end: b_idx + 1,
                    match_score: 0,
                });
                histogram.del_first(line);
            }
        }
    }

    fn fine_solve_region(&self, regions: &[MatchRegion], out: &mut Vec<MatchRegion>) {
        let mut prev_region = None;

        for idx in 0..=regions.len() {
            let region = regions.get(idx).copied();
            let a_offset = prev_region.map(|r: MatchRegion| r.a_end).unwrap_or(0);
            let b_offset = prev_region.map(|r: MatchRegion| r.b_end).unwrap_or(0);
            let aa = &self.aa[a_offset..region.map(|r| r.a_start).unwrap_or(self.aa.len())];
            let bb = &self.bb[b_offset..region.map(|r| r.b_start).unwrap_or(self.bb.len())];

            if let Some(previous) = prev_region {
                out.push(previous);
            }
            prev_region = region;

            if !aa.is_empty() && !bb.is_empty() && aa.len() + bb.len() > 2 {
                let mut local = Vec::new();
                self.find_common_lines(aa, bb, &mut local);
                for mut match_region in local {
                    match_region.a_start += a_offset;
                    match_region.a_end += a_offset;
                    match_region.b_start += b_offset;
                    match_region.b_end += b_offset;
                    out.push(match_region);
                }
            }
        }
    }

    fn solve(&self) -> Vec<DiffPair> {
        let mut regions = Vec::new();
        self.solve_range(0, self.aa.len(), 0, self.bb.len(), &mut regions);

        let mut fine_regions = Vec::new();
        self.fine_solve_region(&regions, &mut fine_regions);

        let mut pairs = Vec::new();
        let mut apos = 0_i32;
        let mut bpos = 0_i32;
        let mut prev_region = MatchRegion {
            a_start: 0,
            a_end: 0,
            b_start: 0,
            b_end: 0,
            match_score: 0,
        };

        for idx in 0..=fine_regions.len() {
            let region = fine_regions.get(idx).copied();
            let a_end = region.map(|r| r.a_start).unwrap_or(self.aa.len());
            let b_end = region.map(|r| r.b_start).unwrap_or(self.bb.len());
            let aa_str = join_lines(&self.aa, prev_region.a_end, a_end);
            let bb_str = join_lines(&self.bb, prev_region.b_end, b_end);
            let a_diff = new_diff(apos, line_len(&aa_str), DiffType::Delete);
            let b_diff = new_diff(bpos, line_len(&bb_str), DiffType::Insert);
            let a_offset = a_diff.offset;
            let b_offset = b_diff.offset;

            if a_diff.length > 0 || b_diff.length > 0 {
                let mut pair = DiffPair::default();
                if a_diff.length > 0 {
                    pair.left = Some(a_diff);
                }
                if b_diff.length > 0 {
                    pair.right = Some(b_diff);
                }

                if !aa_str.is_empty() && !bb_str.is_empty() {
                    let inline_diffs = myers_diff_with_options(
                        &aa_str,
                        &bb_str,
                        DiffOptions {
                            is_array_diff: false,
                            max_edit_length: INLINE_MAX_EDIT_LENGTH,
                        },
                    );
                    let mut classified = classify(&inline_diffs);
                    for diff in &mut classified.left {
                        diff.offset += a_offset;
                    }
                    for diff in &mut classified.right {
                        diff.offset += b_offset;
                    }
                    if let Some(left) = pair.left.as_mut() {
                        left.inline_diffs = classified.left;
                    }
                    if let Some(right) = pair.right.as_mut() {
                        right.inline_diffs = classified.right;
                    }
                }

                pairs.push(pair);
            }

            if let Some(region) = region {
                let aa_common = join_lines(&self.aa, region.a_start, region.a_end);
                let bb_common = join_lines(&self.bb, region.b_start, region.b_end);
                apos += (aa_str.len() + aa_common.len()) as i32;
                bpos += (bb_str.len() + bb_common.len()) as i32;
                prev_region = region;
            }
        }

        pairs
    }
}

pub fn new_diff(offset: i32, length: i32, diff_type: DiffType) -> Diff {
    Diff {
        offset,
        length,
        diff_type,
        inline_diffs: Vec::new(),
    }
}

pub fn sort_diffs(diffs: &mut [Diff]) {
    diffs.sort_by_key(|diff| {
        (
            match diff.diff_type {
                DiffType::Delete => 0,
                DiffType::Insert => 1,
            },
            diff.offset,
        )
    });
}

pub fn classify(diffs: &[Diff]) -> ClassifiedDiffs {
    let mut classified = ClassifiedDiffs::default();
    for diff in diffs {
        match diff.diff_type {
            DiffType::Delete => classified.left.push(diff.clone()),
            DiffType::Insert => classified.right.push(diff.clone()),
        }
    }
    sort_diffs(&mut classified.left);
    sort_diffs(&mut classified.right);
    classified
}

pub fn compare_text(left: &str, right: &str) -> Vec<DiffPair> {
    histogram_diff(left, right)
}

pub fn histogram_diff(left: &str, right: &str) -> Vec<DiffPair> {
    if left.is_empty() && right.is_empty() {
        return Vec::new();
    }
    if left.is_empty() {
        return vec![DiffPair {
            left: None,
            right: Some(new_diff(0, right.len() as i32, DiffType::Insert)),
        }];
    }
    if right.is_empty() {
        return vec![DiffPair {
            left: Some(new_diff(0, left.len() as i32, DiffType::Delete)),
            right: None,
        }];
    }

    HistogramDiffer::new(split_lines(left), split_lines(right)).solve()
}

pub fn myers_diff(left: &str, right: &str) -> Vec<Diff> {
    myers_diff_with_options(left, right, DiffOptions::default())
}

pub fn array_diff(left: &[&str], right: &[&str], options: DiffOptions) -> Vec<Diff> {
    let components = diff_components_tokens(
        left.iter().map(|value| (*value).to_string()).collect(),
        right.iter().map(|value| (*value).to_string()).collect(),
        DiffOptions {
            is_array_diff: true,
            ..options
        },
    );
    let mut left_pos = 0_i32;
    let mut right_pos = 0_i32;
    let mut diffs = Vec::new();

    for component in components {
        let len = component.count as i32;
        if component.removed {
            for _ in 0..len {
                diffs.push(new_diff(left_pos, 1, DiffType::Delete));
                left_pos += 1;
            }
        } else if component.added {
            for _ in 0..len {
                diffs.push(new_diff(right_pos, 1, DiffType::Insert));
                right_pos += 1;
            }
        } else {
            left_pos += len;
            right_pos += len;
        }
    }

    diffs
}

pub fn myers_diff_with_options(left: &str, right: &str, options: DiffOptions) -> Vec<Diff> {
    let components = diff_components(left, right, options);
    let mut left_pos = 0_i32;
    let mut right_pos = 0_i32;
    let mut diffs = Vec::new();

    for component in components {
        let len = component.value.len() as i32;
        if component.removed {
            diffs.push(new_diff(left_pos, len, DiffType::Delete));
            left_pos += len;
        } else if component.added {
            diffs.push(new_diff(right_pos, len, DiffType::Insert));
            right_pos += len;
        } else {
            left_pos += len;
            right_pos += len;
        }
    }

    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let mut idx = 0;
    while idx + 1 < diffs.len() {
        if diffs[idx].diff_type == DiffType::Delete && diffs[idx + 1].diff_type == DiffType::Insert
        {
            while diffs[idx].length > 0
                && diffs[idx + 1].length > 0
                && left_bytes[diffs[idx].offset as usize]
                    == right_bytes[diffs[idx + 1].offset as usize]
            {
                diffs[idx].offset += 1;
                diffs[idx + 1].offset += 1;
                diffs[idx].length -= 1;
                diffs[idx + 1].length -= 1;
            }

            while diffs[idx].length > 0
                && diffs[idx + 1].length > 0
                && left_bytes[(diffs[idx].offset + diffs[idx].length - 1) as usize]
                    == right_bytes[(diffs[idx + 1].offset + diffs[idx + 1].length - 1) as usize]
            {
                diffs[idx].length -= 1;
                diffs[idx + 1].length -= 1;
            }

            idx += 2;
        } else {
            idx += 1;
        }
    }

    diffs.retain(|diff| diff.length != 0);
    diffs
}

fn diff_components(left: &str, right: &str, options: DiffOptions) -> Vec<ComponentValue> {
    let old_array = if options.is_array_diff {
        split_chars(left)
    } else {
        tokenize(left)
    };
    let new_array = if options.is_array_diff {
        split_chars(right)
    } else {
        tokenize(right)
    };
    diff_components_tokens(old_array, new_array, options)
}

fn diff_components_tokens(
    old_array: Vec<String>,
    new_array: Vec<String>,
    options: DiffOptions,
) -> Vec<ComponentValue> {
    let new_len = new_array.len() as i32;
    let old_len = old_array.len() as i32;
    let mut edit_length = 1_i32;
    let mut max_edit_length = new_len + old_len;
    let limit = options.max_edit_length as i32;
    if limit < max_edit_length {
        max_edit_length = limit;
    }

    let max_size = (max_edit_length * 2 + 3) as usize;
    let mut best_path = vec![None; max_size];
    let mut nodes = Vec::new();

    let index = |diagonal: i32| -> usize { (diagonal + max_edit_length + 1) as usize };

    best_path[index(0)] = Some(PathState {
        new_pos: -1,
        last_component: None,
    });
    let Some(base_slot) = best_path[index(0)].as_mut() else {
        return Vec::new();
    };
    let old_pos0 = extract_common(base_slot, &new_array, &old_array, 0, &mut nodes);
    let Some(base_path) = best_path[index(0)].clone() else {
        return Vec::new();
    };
    if base_path.new_pos + 1 >= new_len && old_pos0 + 1 >= old_len {
        return build_values(base_path.last_component, &new_array, &old_array, &nodes);
    }

    while edit_length <= max_edit_length {
        let mut diagonal = -edit_length;
        while diagonal <= edit_length {
            let add_path = best_path[index(diagonal - 1)].clone();
            let remove_path = best_path[index(diagonal + 1)].clone();
            let mut old_pos = remove_path.as_ref().map(|path| path.new_pos).unwrap_or(0);
            old_pos -= diagonal;

            if add_path.is_some() {
                best_path[index(diagonal - 1)] = None;
            }

            let can_add = add_path
                .as_ref()
                .is_some_and(|path| path.new_pos + 1 < new_len);
            let can_remove = remove_path.is_some() && (0..old_len).contains(&old_pos);

            if !can_add && !can_remove {
                best_path[index(diagonal)] = None;
                diagonal += 2;
                continue;
            }

            let use_remove = if !can_add {
                true
            } else if !can_remove {
                false
            } else if let (Some(add_path), Some(remove_path)) = (&add_path, &remove_path) {
                add_path.new_pos < remove_path.new_pos
            } else {
                false
            };

            let mut base = if use_remove {
                if let Some(remove_path) = remove_path {
                    add_to_path(remove_path, false, true, 0, &mut nodes)
                } else {
                    diagonal += 2;
                    continue;
                }
            } else if let Some(add_path) = add_path {
                add_to_path(add_path, true, false, 1, &mut nodes)
            } else {
                diagonal += 2;
                continue;
            };

            old_pos = extract_common(&mut base, &new_array, &old_array, diagonal, &mut nodes);
            if base.new_pos + 1 >= new_len && old_pos + 1 >= old_len {
                return build_values(base.last_component, &new_array, &old_array, &nodes);
            }

            best_path[index(diagonal)] = Some(base);
            diagonal += 2;
        }

        edit_length += 1;
    }

    vec![
        ComponentValue {
            count: 0,
            value: old_array.concat(),
            added: false,
            removed: true,
        },
        ComponentValue {
            count: 0,
            value: new_array.concat(),
            added: true,
            removed: false,
        },
    ]
}

fn add_to_path(
    path: PathState,
    added: bool,
    removed: bool,
    new_pos_inc: i32,
    nodes: &mut Vec<ComponentNode>,
) -> PathState {
    if let Some(last_index) = path.last_component {
        let last = nodes[last_index];
        if last.added == added && last.removed == removed {
            nodes.push(ComponentNode {
                count: last.count + 1,
                added,
                removed,
                previous: last.previous,
            });
            return PathState {
                new_pos: path.new_pos + new_pos_inc,
                last_component: Some(nodes.len() - 1),
            };
        }
    }

    nodes.push(ComponentNode {
        count: 1,
        added,
        removed,
        previous: path.last_component,
    });
    PathState {
        new_pos: path.new_pos + new_pos_inc,
        last_component: Some(nodes.len() - 1),
    }
}

fn extract_common(
    base_path: &mut PathState,
    new_array: &[String],
    old_array: &[String],
    diagonal_path: i32,
    nodes: &mut Vec<ComponentNode>,
) -> i32 {
    let new_len = new_array.len() as i32;
    let old_len = old_array.len() as i32;
    let mut new_pos = base_path.new_pos;
    let mut old_pos = new_pos - diagonal_path;
    let mut common_count = 0_usize;

    while new_pos + 1 < new_len
        && old_pos + 1 < old_len
        && new_array[(new_pos + 1) as usize] == old_array[(old_pos + 1) as usize]
    {
        new_pos += 1;
        old_pos += 1;
        common_count += 1;
    }

    if common_count != 0 {
        nodes.push(ComponentNode {
            count: common_count,
            added: false,
            removed: false,
            previous: base_path.last_component,
        });
        base_path.last_component = Some(nodes.len() - 1);
    }

    base_path.new_pos = new_pos;
    old_pos
}

fn build_values(
    last_component: Option<usize>,
    new_array: &[String],
    old_array: &[String],
    nodes: &[ComponentNode],
) -> Vec<ComponentValue> {
    let mut component_indices = Vec::new();
    let mut current = last_component;
    while let Some(index) = current {
        component_indices.push(index);
        current = nodes[index].previous;
    }
    component_indices.reverse();

    let mut out = Vec::new();
    let mut new_pos = 0_usize;
    let mut old_pos = 0_usize;

    for component_index in component_indices {
        let component = nodes[component_index];
        if !component.removed {
            if !component.added {
                let slice = &new_array[new_pos..new_pos + component.count];
                let old_slice = &old_array[old_pos..old_pos + component.count];
                let mut value = String::new();
                for (new_token, old_token) in slice.iter().zip(old_slice.iter()) {
                    if old_token.len() > new_token.len() {
                        value.push_str(old_token);
                    } else {
                        value.push_str(new_token);
                    }
                }
                out.push(ComponentValue {
                    count: component.count,
                    value,
                    added: false,
                    removed: false,
                });
            } else {
                out.push(ComponentValue {
                    count: component.count,
                    value: new_array[new_pos..new_pos + component.count].concat(),
                    added: true,
                    removed: false,
                });
            }
            new_pos += component.count;
            if !component.added {
                old_pos += component.count;
            }
        } else {
            out.push(ComponentValue {
                count: component.count,
                value: old_array[old_pos..old_pos + component.count].concat(),
                added: false,
                removed: true,
            });
            old_pos += component.count;
            if out.len() > 1 && out[out.len() - 2].added {
                let last = out.len() - 1;
                out.swap(last - 1, last);
            }
        }
    }

    if out.len() > 1 {
        if let Some(last) = out.last().cloned() {
            if (last.added || last.removed) && last.value.is_empty() {
                let prev = out.len() - 2;
                out[prev].value.push_str(&last.value);
                out.pop();
            }
        }
    }

    out
}

fn split_chars(value: &str) -> Vec<String> {
    value.chars().map(|ch| ch.to_string()).collect()
}

fn tokenize(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token_start = None;
    let mut prev_is_word = None;
    let mut i = 0;

    while i < value.len() {
        let Some(ch) = value[i..].chars().next() else {
            break;
        };
        let len = ch.len_utf8();

        if is_whitespace_not_newline(ch) {
            if let Some(start) = token_start.take() {
                tokens.push(value[start..i].to_string());
            }
            let mut end = i + len;
            while end < value.len() {
                let Some(next) = value[end..].chars().next() else {
                    break;
                };
                if !is_whitespace_not_newline(next) {
                    break;
                }
                end += next.len_utf8();
            }
            tokens.push(value[i..end].to_string());
            i = end;
            prev_is_word = None;
            continue;
        }

        if is_newline(ch) || is_punct(ch) {
            if let Some(start) = token_start.take() {
                tokens.push(value[start..i].to_string());
            }
            tokens.push(value[i..i + len].to_string());
            i += len;
            prev_is_word = None;
            continue;
        }

        let is_word = is_word_char(ch);
        if let Some(previous) = prev_is_word {
            if previous != is_word {
                if let Some(start) = token_start {
                    tokens.push(value[start..i].to_string());
                }
                tokens.push(String::new());
                token_start = Some(i);
            }
        } else if token_start.is_none() {
            token_start = Some(i);
        }

        prev_is_word = Some(is_word);
        i += len;
    }

    if let Some(start) = token_start {
        tokens.push(value[start..].to_string());
    }

    let mut idx = 0;
    while idx + 2 < tokens.len() {
        if tokens[idx + 1].is_empty()
            && is_extended_word_chars(&tokens[idx])
            && is_extended_word_chars(&tokens[idx + 2])
        {
            let merged = format!("{}{}", tokens[idx], tokens[idx + 2]);
            tokens[idx] = merged;
            tokens.remove(idx + 1);
            tokens.remove(idx + 1);
            idx = idx.saturating_sub(1);
        } else {
            idx += 1;
        }
    }

    tokens
        .into_iter()
        .filter(|token| !token.is_empty())
        .collect()
}

fn split_lines(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (idx, byte) in input.bytes().enumerate() {
        if byte == b'\n' {
            out.push(&input[start..idx]);
            start = idx + 1;
        }
    }
    out.push(&input[start..]);
    out
}

fn join_lines(lines: &[&str], start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }

    let mut out = String::new();
    for (idx, line) in lines[start..end].iter().enumerate() {
        if idx != 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    out.push('\n');
    out
}

fn line_len(line: &str) -> i32 {
    if line.is_empty() {
        0
    } else if line.len() > 1 {
        (line.len() - 1) as i32
    } else {
        1
    }
}

fn trim_line(line: &str) -> &str {
    line.trim_matches(|ch| matches!(ch, ' ' | '\t' | '\r' | '\n'))
}

fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_whitespace_not_newline(ch: char) -> bool {
    ch.is_ascii_whitespace() && !matches!(ch, '\r' | '\n')
}

fn is_newline(ch: char) -> bool {
    matches!(ch, '\r' | '\n')
}

fn is_punct(ch: char) -> bool {
    matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '\'' | '"')
}

fn is_extended_word_chars(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    value.chars().all(|ch| {
        ch.is_ascii_alphabetic()
            || ('\u{00C0}'..='\u{00FF}').contains(&ch)
            || ('\u{00D8}'..='\u{00F6}').contains(&ch)
            || ('\u{00F8}'..='\u{02C6}').contains(&ch)
            || ('\u{02C8}'..='\u{02D7}').contains(&ch)
            || ('\u{02DE}'..='\u{02FF}').contains(&ch)
            || ('\u{1E00}'..='\u{1EFF}').contains(&ch)
    })
}
