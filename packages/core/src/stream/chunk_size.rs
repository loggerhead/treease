/// Default chunk size for streaming: 128KB.
///
/// 128KB is the balanced optimum across all size buckets
/// (benchmarked with 5 candidates × 4 size buckets).
pub const DEFAULT_CHUNK_SIZE: usize = 128 * 1024;

/// Threshold above which a larger chunk size is used.
pub const LARGE_FILE_THRESHOLD: usize = 4 * 1024 * 1024;

/// Chunk size for files >= 4MB: 256KB.
pub const LARGE_FILE_CHUNK_SIZE: usize = 256 * 1024;

/// Select an optimal chunk size based on total input bytes.
///
/// Larger documents benefit from bigger chunks to reduce per-chunk overhead;
/// smaller documents use the default chunk size for smoother progress updates.
pub fn select_chunk_size(total_bytes: usize) -> usize {
    if total_bytes >= LARGE_FILE_THRESHOLD {
        LARGE_FILE_CHUNK_SIZE
    } else {
        DEFAULT_CHUNK_SIZE
    }
}
