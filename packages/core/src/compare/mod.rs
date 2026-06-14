mod algorithms;
pub mod compare;
pub mod diff;
pub mod histogram;
pub mod myers;

pub use compare::compare_text;
pub use diff::{ClassifiedDiffs, Diff, DiffPair, DiffType, classify, new_diff};
pub use histogram::histogram_diff;
pub use myers::{DiffOptions, MAX_EDIT_LENGTH, array_diff, myers_diff, myers_diff_with_options};
pub mod structured;
pub use structured::compare_texts_structured;
pub use structured::diff_texts_structured;
