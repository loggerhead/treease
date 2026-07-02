/// Default chunk size for streaming: 128KB.
///
/// 128KB is the balanced optimum across all size buckets
/// (benchmarked with 5 candidates × 4 size buckets).
pub const DEFAULT_CHUNK_SIZE: usize = 128 * 1024;

/// Threshold above which a larger chunk size is used.
pub const LARGE_FILE_THRESHOLD: usize = 4 * 1024 * 1024;

/// Chunk size for files >= 4MB: 256KB.
pub const LARGE_FILE_CHUNK_SIZE: usize = 256 * 1024;

/// Threshold above which an extra-large chunk size is used.
pub const HUGE_FILE_THRESHOLD: usize = 10 * 1024 * 1024;

/// Chunk size for files > 10MB: 1MB.
pub const HUGE_FILE_CHUNK_SIZE: usize = 1024 * 1024;

/// Select an optimal chunk size based on total input bytes.
///
/// Larger documents benefit from bigger chunks to reduce per-chunk overhead;
/// smaller documents use the default chunk size for smoother progress updates.
pub fn select_chunk_size(total_bytes: usize) -> usize {
    if total_bytes > HUGE_FILE_THRESHOLD {
        HUGE_FILE_CHUNK_SIZE
    } else if total_bytes >= LARGE_FILE_THRESHOLD {
        LARGE_FILE_CHUNK_SIZE
    } else {
        DEFAULT_CHUNK_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_CHUNK_SIZE, HUGE_FILE_CHUNK_SIZE, HUGE_FILE_THRESHOLD, LARGE_FILE_CHUNK_SIZE,
        LARGE_FILE_THRESHOLD, select_chunk_size,
    };

    #[test]
    fn uses_default_chunk_size_below_large_threshold() {
        assert_eq!(
            select_chunk_size(LARGE_FILE_THRESHOLD - 1),
            DEFAULT_CHUNK_SIZE
        );
    }

    #[test]
    fn uses_large_chunk_size_from_large_threshold_through_huge_threshold() {
        assert_eq!(
            select_chunk_size(LARGE_FILE_THRESHOLD),
            LARGE_FILE_CHUNK_SIZE
        );
        assert_eq!(
            select_chunk_size(HUGE_FILE_THRESHOLD),
            LARGE_FILE_CHUNK_SIZE
        );
    }

    #[test]
    fn uses_huge_chunk_size_above_huge_threshold() {
        assert_eq!(
            select_chunk_size(HUGE_FILE_THRESHOLD + 1),
            HUGE_FILE_CHUNK_SIZE
        );
    }
}
