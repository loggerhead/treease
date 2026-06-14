use treease_core::compare::{
    DiffOptions, DiffPair, DiffType, array_diff, histogram_diff, myers_diff,
    myers_diff_with_options, new_diff,
};

#[test]
fn compare_algorithms_myers_returns_empty_for_identical_strings() {
    assert!(myers_diff("abc", "abc").is_empty());
}

#[test]
fn compare_algorithms_myers_isolates_middle_replacement_without_consuming_shared_prefix_or_suffix()
{
    let diffs = myers_diff("abcXYZdef", "abcQdef");

    assert_eq!(diffs.len(), 2);
    assert_eq!(diffs[0].diff_type, DiffType::Delete);
    assert_eq!(diffs[0].offset, 3);
    assert_eq!(diffs[0].length, 3);
    assert_eq!(diffs[1].diff_type, DiffType::Insert);
    assert_eq!(diffs[1].offset, 3);
    assert_eq!(diffs[1].length, 1);
}

#[test]
fn compare_algorithms_myers_keeps_single_character_replacement_narrow() {
    let diffs = myers_diff("abc", "adc");

    assert_eq!(diffs.len(), 2);
    assert_eq!(diffs[0].diff_type, DiffType::Delete);
    assert_eq!(diffs[0].offset, 1);
    assert_eq!(diffs[0].length, 1);
    assert_eq!(diffs[1].diff_type, DiffType::Insert);
    assert_eq!(diffs[1].offset, 1);
    assert_eq!(diffs[1].length, 1);
}

#[test]
fn compare_algorithms_myers_falls_back_to_whole_range_replace_when_max_edit_length_is_too_small() {
    let options = DiffOptions {
        max_edit_length: 1,
        ..Default::default()
    };
    let diffs = myers_diff_with_options("abcdefghij", "1234567890", options);

    assert_eq!(diffs.len(), 2);
    assert_eq!(diffs[0].diff_type, DiffType::Delete);
    assert_eq!(diffs[0].offset, 0);
    assert_eq!(diffs[0].length, 10);
    assert_eq!(diffs[1].diff_type, DiffType::Insert);
    assert_eq!(diffs[1].offset, 0);
    assert_eq!(diffs[1].length, 10);
}

#[test]
fn compare_algorithms_array_diff_reports_delete_and_insert_around_repeated_elements() {
    let left = ["a", "b", "a", "c"];
    let right = ["a", "a", "c", "d"];
    let diffs = array_diff(&left, &right, DiffOptions::default());

    assert_eq!(diffs.len(), 2);
    assert_eq!(diffs[0].diff_type, DiffType::Delete);
    assert_eq!(diffs[0].offset, 1);
    assert_eq!(diffs[0].length, 1);
    assert_eq!(diffs[1].diff_type, DiffType::Insert);
    assert_eq!(diffs[1].offset, 3);
    assert_eq!(diffs[1].length, 1);
}

#[test]
fn compare_algorithms_myers_reports_middle_token_replacement_as_delete_then_insert() {
    let diffs = myers_diff("abc\nold\ndef\n", "abc\nnew\ndef\n");

    assert_eq!(diffs.len(), 2);
    assert_eq!(diffs[0].diff_type, DiffType::Delete);
    assert_eq!(diffs[0].offset, 4);
    assert_eq!(diffs[0].length, 3);
    assert_eq!(diffs[1].diff_type, DiffType::Insert);
    assert_eq!(diffs[1].offset, 4);
    assert_eq!(diffs[1].length, 3);
}

#[test]
fn compare_algorithms_histogram_treats_repeated_lines_as_localized_edits() {
    let pairs = histogram_diff("a\nx\na\ny\na\n", "a\ny\na\nx\na\n");

    assert_eq!(pairs.len(), 2);
    assert!(pairs[0].left.is_some());
    assert!(pairs[0].right.is_none());
    assert!(pairs[1].left.is_none());
    assert!(pairs[1].right.is_some());
}

#[test]
fn compare_algorithms_histogram_ignores_outer_line_whitespace_through_trim_line_matching() {
    let pairs = histogram_diff("alpha\n  beta  \ngamma\n", "alpha\nbeta\ngamma\n");

    assert_eq!(pairs.len(), 0);
}

#[test]
fn compare_algorithms_histogram_returns_insert_pair_when_left_is_empty() {
    let pairs = histogram_diff("", "hello\nworld\n");

    assert_eq!(
        pairs,
        vec![DiffPair {
            left: None,
            right: Some(new_diff(0, 12, DiffType::Insert)),
        }]
    );
}

#[test]
fn compare_algorithms_histogram_returns_delete_pair_when_right_is_empty() {
    let pairs = histogram_diff("hello\nworld\n", "");

    assert_eq!(
        pairs,
        vec![DiffPair {
            left: Some(new_diff(0, 12, DiffType::Delete)),
            right: None,
        }]
    );
}
