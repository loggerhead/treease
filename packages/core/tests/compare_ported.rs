use treease_core::compare::diff::sort_diffs;
use treease_core::compare::{
    Diff, DiffPair, DiffType, compare_text, histogram_diff, myers_diff, new_diff,
};

#[test]
fn text_compare_empty_inputs_have_no_diffs() {
    assert!(compare_text("", "").is_empty());
}

#[test]
fn text_compare_reports_whole_left_deletion_for_empty_right() {
    let pairs = compare_text("12345", "");

    assert_eq!(
        pairs,
        vec![DiffPair {
            left: Some(new_diff(0, 5, DiffType::Delete)),
            right: None,
        }]
    );
}

#[test]
fn text_compare_reports_whole_right_insertion_for_empty_left() {
    let pairs = compare_text("", "12345");

    assert_eq!(
        pairs,
        vec![DiffPair {
            left: None,
            right: Some(new_diff(0, 5, DiffType::Insert)),
        }]
    );
}

#[test]
fn text_compare_equal_non_empty_multiline_has_no_hunks() {
    let text = "{\n  \"a\": 1,\n  \"b\": [true, false]\n}";

    assert!(compare_text(text, text).is_empty());
}

#[test]
fn text_compare_reports_changed_middle_lines_after_common_edges() {
    let left = "same\nold\nsame";
    let right = "same\nnew\nsame";

    assert_eq!(
        flatten_pairs(compare_text(left, right)),
        vec![
            new_diff(5, 3, DiffType::Delete),
            new_diff(5, 3, DiffType::Insert),
        ]
    );
}

#[test]
fn text_compare_reports_inserted_line_after_shared_prefix() {
    let pairs = compare_text("alpha\nomega\n", "alpha\nnew\nomega\n");

    assert_eq!(
        pairs,
        vec![DiffPair {
            left: None,
            right: Some(new_diff(6, 3, DiffType::Insert)),
        }]
    );
}

#[test]
fn text_compare_ignores_crlf_lf_only_differences() {
    let left = "{\r\n  \"a\": 1\r\n}";
    let right = "{\n  \"a\": 1\n}";

    let pairs = compare_text(left, right);

    assert!(pairs.is_empty());
}

#[test]
fn myers_diff_flattens_middle_line_replacement() {
    let diffs = myers_diff("abc\nold\ndef\n", "abc\nnew\ndef\n");

    assert_eq!(
        diffs,
        vec![
            new_diff(4, 3, DiffType::Delete),
            new_diff(4, 3, DiffType::Insert),
        ]
    );
}

#[test]
fn histogram_diff_collapses_contiguous_middle_block_replacement() {
    let left = "header\nold-1\nold-2\nfooter\n";
    let right = "header\nnew-1\nnew-2\nfooter\n";

    assert_eq!(
        flatten_pairs(histogram_diff(left, right)),
        vec![
            new_diff(7, 11, DiffType::Delete),
            new_diff(7, 11, DiffType::Insert),
        ]
    );
}

#[test]
fn histogram_diff_preserves_shared_suffix_for_middle_deletion() {
    let left = "alpha\nremove-a\nremove-b\nomega\n";
    let right = "alpha\nomega\n";

    assert_eq!(
        histogram_diff(left, right),
        vec![DiffPair {
            left: Some(new_diff(6, 17, DiffType::Delete)),
            right: None,
        }]
    );
}

#[test]
fn histogram_diff_ignores_crlf_lf_only_differences() {
    let left = "{\r\n  \"a\": 1\r\n  \"b\": 2\r\n}";
    let right = "{\n  \"a\": 1\n  \"b\": 2\n}";

    assert!(histogram_diff(left, right).is_empty());
}

#[test]
fn histogram_diff_counts_inserted_multiline_block_from_empty_left() {
    let pairs = histogram_diff("", "hello\nworld\n");

    assert_eq!(
        pairs,
        vec![DiffPair {
            left: None,
            right: Some(new_diff(0, 12, DiffType::Insert)),
        }]
    );
}

fn flatten_pairs(pairs: Vec<DiffPair>) -> Vec<Diff> {
    let mut diffs = Vec::new();
    for pair in pairs {
        if let Some(mut left) = pair.left {
            left.inline_diffs.clear();
            diffs.push(left);
        }
        if let Some(mut right) = pair.right {
            right.inline_diffs.clear();
            diffs.push(right);
        }
    }
    diffs
}

fn collect_hunks(pairs: &[DiffPair]) -> Vec<Diff> {
    let mut hunks = Vec::new();
    for pair in pairs {
        if let Some(left) = &pair.left {
            let mut h = left.clone();
            h.inline_diffs.clear();
            hunks.push(h);
        }
        if let Some(right) = &pair.right {
            let mut h = right.clone();
            h.inline_diffs.clear();
            hunks.push(h);
        }
    }
    sort_diffs(&mut hunks);
    hunks
}

fn collect_inlines(pairs: &[DiffPair]) -> Vec<Diff> {
    let mut inlines = Vec::new();
    for pair in pairs {
        if let Some(left) = &pair.left {
            inlines.extend(left.inline_diffs.clone());
        }
        if let Some(right) = &pair.right {
            inlines.extend(right.inline_diffs.clone());
        }
    }
    sort_diffs(&mut inlines);
    inlines
}

// ===== Tests ported from diff_text.zig =====

#[test]
fn text_compare_human_readable_case() {
    let left = r#"{
    "Aidan Gillen": {
        "array": [
            "Game of Thron\\"es",
            "The Wire"
        ],
        "string": "some string",
        "int": 2,
        "aboolean": true,
        "boolean": true,
        "null": null,
        "a_null": null,
        "another_null": "null check",
        "object": {
            "foo": "bar",
            "object1": {
                "new prop1": "new prop value"
            },
            "object2": {
                "new prop1": "new prop value"
            },
            "object3": {
                "new prop1": "new prop value"
            },
            "object4": {
                "new prop1": "new prop value"
            }
        }
    },
    "Amy Ryan": {
        "one": "In Treatment",
        "two": "The Wire"
    },
    "Annie Fitzgerald": [
        "Big Love",
        "True Blood"
    ],
    "Anwan Glover": [
        "Treme",
        "The Wire"
    ],
    "Alexander Skarsgard": [
        "Generation Kill",
        "True Blood"
    ],
    "Clarke Peters": null
}"#;
    let right = r#"{
    "Aidan Gillen": {
        "array": [
            "Game of Thrones",
            "The Wire"
        ],
        "string": "some string",
        "int": "2",
        "otherint": 4,
        "aboolean": "true",
        "boolean": false,
        "null": null,
        "a_null": 88,
        "another_null": null,
        "object": {
            "foo": "bar"
        }
    },
    "Amy Ryan": [
        "In Treatment",
        "The Wire"
    ],
    "Annie Fitzgerald": [
        "True Blood",
        "Big Love",
        "The Sopranos",
        "Oz"
    ],
    "Anwan Glover": [
        "Treme",
        "The Wire"
    ],
    "Alexander Skarsg?rd": [
        "Generation Kill",
        "True Blood"
    ],
    "Alice Farmer": [
        "The Corner",
        "Oz",
        "The Wire"
    ]
}"#;

    let pairs = compare_text(left, right);
    let hunks = collect_hunks(&pairs);
    let inlines = collect_inlines(&pairs);

    assert_eq!(
        hunks,
        vec![
            new_diff(43, 33, DiffType::Delete),
            new_diff(144, 68, DiffType::Delete),
            new_diff(235, 61, DiffType::Delete),
            new_diff(317, 368, DiffType::Delete),
            new_diff(703, 81, DiffType::Delete),
            new_diff(831, 20, DiffType::Delete),
            new_diff(924, 28, DiffType::Delete),
            new_diff(1008, 25, DiffType::Delete),
            new_diff(43, 30, DiffType::Insert),
            new_diff(141, 96, DiffType::Insert),
            new_diff(260, 51, DiffType::Insert),
            new_diff(332, 24, DiffType::Insert),
            new_diff(374, 67, DiffType::Insert),
            new_diff(468, 21, DiffType::Insert),
            new_diff(510, 36, DiffType::Insert),
            new_diff(619, 28, DiffType::Insert),
            new_diff(703, 82, DiffType::Insert),
        ]
    );
    assert_eq!(
        inlines,
        vec![
            new_diff(69, 2, DiffType::Delete),
            new_diff(72, 3, DiffType::Delete),
            new_diff(207, 3, DiffType::Delete),
            new_diff(253, 4, DiffType::Delete),
            new_diff(283, 1, DiffType::Delete),
            new_diff(288, 7, DiffType::Delete),
            new_diff(341, 344, DiffType::Delete),
            new_diff(719, 1, DiffType::Delete),
            new_diff(730, 7, DiffType::Delete),
            new_diff(761, 7, DiffType::Delete),
            new_diff(782, 1, DiffType::Delete),
            new_diff(841, 2, DiffType::Delete),
            new_diff(845, 5, DiffType::Delete),
            new_diff(945, 1, DiffType::Delete),
            new_diff(1013, 5, DiffType::Delete),
            new_diff(1020, 6, DiffType::Delete),
            new_diff(1029, 4, DiffType::Delete),
            new_diff(69, 2, DiffType::Insert),
            new_diff(156, 1, DiffType::Insert),
            new_diff(158, 1, DiffType::Insert),
            new_diff(170, 23, DiffType::Insert),
            new_diff(204, 1, DiffType::Insert),
            new_diff(209, 1, DiffType::Insert),
            new_diff(231, 4, DiffType::Insert),
            new_diff(278, 2, DiffType::Insert),
            new_diff(390, 1, DiffType::Insert),
            new_diff(439, 1, DiffType::Insert),
            new_diff(520, 1, DiffType::Insert),
            new_diff(523, 8, DiffType::Insert),
            new_diff(532, 1, DiffType::Insert),
            new_diff(534, 13, DiffType::Insert),
            new_diff(640, 1, DiffType::Insert),
            new_diff(708, 4, DiffType::Insert),
            new_diff(714, 6, DiffType::Insert),
            new_diff(723, 1, DiffType::Insert),
            new_diff(725, 61, DiffType::Insert),
        ]
    );
}

#[test]
fn text_compare_char_compare() {
    assert!(compare_text("", "").is_empty());

    let pairs = compare_text("  \"foo\": \"abc\" }", "{ \"foo\": \"adc\" }");
    assert_eq!(
        collect_inlines(&pairs),
        vec![
            new_diff(0, 1, DiffType::Delete),
            new_diff(11, 1, DiffType::Delete),
            new_diff(0, 1, DiffType::Insert),
            new_diff(11, 1, DiffType::Insert),
        ]
    );

    let left2 = "[\n     ,\n    2\n ";
    let right2 = "[\n    1,\n    2\n]";
    let pairs2 = compare_text(left2, right2);
    assert_eq!(
        collect_inlines(&pairs2),
        vec![
            new_diff(6, 1, DiffType::Delete),
            new_diff(15, 1, DiffType::Delete),
            new_diff(6, 1, DiffType::Insert),
            new_diff(15, 1, DiffType::Insert),
        ]
    );
}

#[test]
fn text_compare_a_to_a2345_both_hunk_and_inline() {
    let pairs = compare_text("a", "a2345");

    assert_eq!(
        collect_hunks(&pairs),
        vec![
            new_diff(0, 1, DiffType::Delete),
            new_diff(0, 5, DiffType::Insert),
        ]
    );
    assert_eq!(
        collect_inlines(&pairs),
        vec![new_diff(1, 4, DiffType::Insert)]
    );
}

#[test]
fn text_compare_viewzone_error_case_1() {
    // Regression: extra blank line should produce one hunk deletion.
    let left = "{\n\n  return tokens;\n}";
    let right = "{\n  return tokens;\n}";

    let pairs = compare_text(left, right);
    let hunks = collect_hunks(&pairs);

    assert_eq!(
        hunks.len(),
        1,
        "expected 1 hunk, got {}: {:?}",
        hunks.len(),
        hunks
    );

    assert_eq!(hunks[0].diff_type, DiffType::Delete);
    assert_eq!(hunks[0].offset, 2);
    assert_eq!(hunks[0].length, 1);
}

#[test]
fn text_compare_viewzone_error_case_2() {
    let left = r#"wordDiff.tokenize = function(value) {
  // All whitespace symbols except newline group into one token, each newline - in separate token
  let tokens = value.split(/([^\S\r\n]+|[()[\]{}'"\r\n]|\b)/);

  // Join the boundary splits that we do not consider to be boundaries. This is primarily the extended Latin character set.
  for (let i = 0; i < tokens.length - 1; i++) {
    // If we have an empty string in the next field and we have only word chars before and after, merge
    if (!tokens[i + 1] && tokens[i + 2]
          && extendedWordChars.test(tokens[i])
          && extendedWordChars.test(tokens[i + 2])) {
      tokens[i] += tokens[i + 2];
      tokens.splice(i + 1, 2);
      i--;
    }
  }

  return tokens;
};"#;
    let right = r#"wordDiff.tokenize = function(value) {
  const tokens = [];
  let prevCharType = '';
  for (let i = 0; i < value.length; i++) {
    const char = value[i];
    if (spaceRegExp.test(char)) {
      if(prevCharType === 'space') {
        tokens[tokens.length - 1] += ' ';
      } else {
        tokens.push(' ');
      }
      prevCharType = 'space';
    } else if (cannotBecomeWordRegExp.test(char)) {
      tokens.push(char);
      prevCharType = '';
    } else {
      if(prevCharType === 'word') {
        tokens[tokens.length - 1] += char;
      } else {
        tokens.push(char);
      }
      prevCharType = 'word';
    }
  }
  return tokens;
};"#;

    assert_eq!(
        collect_hunks(&compare_text(left, right)),
        vec![
            new_diff(38, 654, DiffType::Delete),
            new_diff(703, 1, DiffType::Delete),
            new_diff(38, 580, DiffType::Insert),
        ]
    );
}

#[test]
fn text_compare_diff_error() {
    let left = r#"hello


b
    }
  }

  return tokens;
};"#;
    let right = r#"world

a
c
            } ;
        }
    } return tokens;
};"#;

    assert_eq!(
        collect_hunks(&compare_text(left, right)),
        vec![
            new_diff(0, 5, DiffType::Delete),
            new_diff(7, 2, DiffType::Delete),
            new_diff(16, 21, DiffType::Delete),
            new_diff(0, 5, DiffType::Insert),
            new_diff(7, 19, DiffType::Insert),
            new_diff(37, 20, DiffType::Insert),
        ]
    );
}

#[test]
fn text_compare_equal_non_empty_multiline_has_no_hunks_and_no_inline_diffs() {
    let text = "\
{\n\
  \"a\": 1,\n\
  \"b\": [true, false],\n\
  \"c\": { \"x\": \"y\" }\n\
}";

    let pairs = compare_text(text, text);

    assert_eq!(collect_hunks(&pairs), Vec::<Diff>::new());
    assert_eq!(collect_inlines(&pairs), Vec::<Diff>::new());
}

#[test]
fn text_compare_short_input_produces_both_hunks_and_inlines() {
    let left = "\
hello\n\
\n\
\n\
b\n\
    }\n\
  }\n\
\n\
  return tokens;\n\
};";
    let right = "\
world\n\
\n\
a\n\
c\n\
            } ;\n\
        }\n\
    } return tokens;\n\
};";

    // Same input as text_compare_diff_error above.
    assert!(left.len() < 100, "left input should be < 100 chars");
    assert!(right.len() < 100, "right input should be < 100 chars");

    let pairs = compare_text(left, right);
    let hunks = collect_hunks(&pairs);
    let inlines = collect_inlines(&pairs);

    assert!(!hunks.is_empty(), "expected at least one hunk");
    assert!(!inlines.is_empty(), "expected at least one inline diff");

    let has_del = hunks.iter().any(|h| h.diff_type == DiffType::Delete);
    let has_ins = hunks.iter().any(|h| h.diff_type == DiffType::Insert);
    assert!(has_del && has_ins, "expected both delete and insert hunks");
}

#[test]
fn text_compare_inline_token_keeps_numeric_deletion_and_tags_replacement_local() {
    let left = "\
{\n\
  \"ratio\": 0.125,\n\
  \"tags\": [\"alpha\", \"beta\", \"gamma\"]\n\
}";
    let right = "\
{\n\
  \"ratio\": 0.15,\n\
  \"tags\": [\"a9aaaaaaa\", \"beta\", \"gamma\"]\n\
}";

    let pairs = compare_text(left, right);

    let mut ratio_has_single_digit_del = false;
    let mut tags_inline_does_not_cover_key = true;
    let mut tags_left_replacement_ok = false;
    let mut tags_right_replacement_ok = false;

    for pair in &pairs {
        if let Some(ld) = &pair.left {
            let lstart = ld.offset as usize;
            let lend = (ld.offset + ld.length) as usize;
            if lend > left.len() {
                continue;
            }
            let lchunk = &left[lstart..lend];

            // Verify that the "ratio" key's inline diff includes a single-char
            // deletion (the "2" in "0.125" changing to "0.15").
            if lchunk.contains("\"ratio\"") {
                for id in &ld.inline_diffs {
                    if id.diff_type == DiffType::Delete && id.length == 1 {
                        let ds = id.offset as usize;
                        let de = (id.offset + id.length) as usize;
                        if de <= left.len() && &left[ds..de] == "2" {
                            ratio_has_single_digit_del = true;
                        }
                    }
                }
            }

            // Verify that the "tags" key's inline diffs do NOT cover the key
            // itself -- only the value content ("alpha") should be in the diff.
            if let Some(key_pos) = lchunk.find("\"tags\"") {
                let abs_key_pos = lstart + key_pos;
                let key_end = abs_key_pos + 6; // length of "\"tags\""

                for id in &ld.inline_diffs {
                    if id.diff_type != DiffType::Delete {
                        continue;
                    }
                    let ds = id.offset as usize;
                    let de = (id.offset + id.length) as usize;
                    // The inline diff should NOT overlap with the tag key.
                    if ds < key_end && de > abs_key_pos {
                        tags_inline_does_not_cover_key = false;
                    }
                    if de <= left.len() && left[ds..de].contains("lph") {
                        tags_left_replacement_ok = true;
                    }
                }
            }
        }

        if let Some(rd) = &pair.right {
            let rstart = rd.offset as usize;
            let rend = (rd.offset + rd.length) as usize;
            if rend > right.len() {
                continue;
            }
            let rchunk = &right[rstart..rend];

            // Verify that the "tags" key's insert inline diffs do NOT cover
            // the key itself.
            if let Some(key_pos) = rchunk.find("\"tags\"") {
                let abs_key_pos = rstart + key_pos;
                let key_end = abs_key_pos + 6;

                for id in &rd.inline_diffs {
                    if id.diff_type != DiffType::Insert {
                        continue;
                    }
                    let ds = id.offset as usize;
                    let de = (id.offset + id.length) as usize;
                    if ds < key_end && de > abs_key_pos {
                        tags_inline_does_not_cover_key = false;
                    }
                    if de <= right.len() && right[ds..de].contains("9aaaaaa") {
                        tags_right_replacement_ok = true;
                    }
                }
            }
        }
    }

    assert!(
        ratio_has_single_digit_del,
        "expected single-digit deletion ('2') in ratio inline diffs"
    );
    assert!(
        tags_inline_does_not_cover_key,
        "tags inline diffs should not cover the tag key itself"
    );
    assert!(
        tags_left_replacement_ok,
        "expected left tags inline diffs to isolate the replaced alpha slice"
    );
    assert!(
        tags_right_replacement_ok,
        "expected right tags inline diffs to isolate the inserted replacement slice"
    );
}

#[test]
fn text_compare_diff_error_3() {
    // US/CN region+currency comparison: the Zig test passes null for both
    // hunks and inlines (don't-check), so the intent is just to verify
    // compare_text handles this input without crashing. The lines differ
    // so a diff pair is expected -- this is a smoke test, not a zero-diff
    // assertion.
    let left = "\
{\n\
  \"region\": \"US\",\n\
  \"currency\": \"USD\"\n\
}";
    let right = "\
{\n\
  \"region\": \"CN\",\n\
  \"currency\": \"CNY\"\n\
}";

    let pairs = compare_text(left, right);
    // Just verify the function runs without panic -- the Zig test
    // intentionally passes null for both expected hunks and inlines.
    assert_eq!(
        pairs.len(),
        1,
        "expected 1 pair for differing values, got {}",
        pairs.len()
    );
}

#[test]
fn text_compare_ignores_tab_space_only_differences() {
    let left = "{\n  \"a\": 1\n}";
    let right = "{\n\t\"a\": 1\n}";

    let pairs = compare_text(left, right);
    assert!(
        pairs.is_empty(),
        "expected 0 pairs for tab-space only differences, got {}",
        pairs.len()
    );
}
