pub const DEFAULT_SCANNER_COMPACT_THRESHOLD: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonScannerConfig {
    pub compact_threshold: usize,
}

impl Default for JsonScannerConfig {
    fn default() -> Self {
        Self {
            compact_threshold: DEFAULT_SCANNER_COMPACT_THRESHOLD,
        }
    }
}

pub fn scanner_compact_threshold() -> usize {
    JsonScannerConfig::default().compact_threshold
}
