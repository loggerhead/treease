use super::super::protocol::{DocumentJobSpec, JobTerminal, SnapshotId};
use super::super::stream_state::StreamState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DocumentJobHandle(pub u64);

#[derive(Debug)]
pub(crate) struct JobEntry {
    pub(crate) spec: DocumentJobSpec,
    pub(crate) request_seq: u64,
    pub(crate) latest_snapshot_id: Option<SnapshotId>,
    pub(crate) terminal: Option<JobTerminal>,
    pub(crate) source_buffer: Option<String>,
    pub(crate) source_bytes: Option<Vec<u8>>,
    /// Append-only line count from streaming chunks.
    pub(crate) line_count: u32,
    pub(crate) stream_state: Option<StreamState>,
}

impl Default for JobEntry {
    fn default() -> Self {
        Self {
            spec: DocumentJobSpec::default(),
            request_seq: 0,
            latest_snapshot_id: None,
            terminal: None,
            source_buffer: None,
            source_bytes: None,
            line_count: 0,
            stream_state: None,
        }
    }
}

impl JobEntry {
    pub(crate) fn append_source_text(&mut self, text: &str) {
        if let Some(bytes) = self.source_bytes.as_mut() {
            bytes.extend_from_slice(text.as_bytes());
            return;
        }
        self.source_buffer
            .get_or_insert_with(String::new)
            .push_str(text);
    }

    pub(crate) fn append_source_bytes(&mut self, bytes: &[u8]) {
        let buffer = self
            .source_bytes
            .get_or_insert_with(|| self.source_buffer.take().unwrap_or_default().into_bytes());
        buffer.extend_from_slice(bytes);
    }

    #[allow(dead_code)]
    pub(crate) fn source_len(&self) -> u32 {
        self.source_bytes
            .as_ref()
            .map_or_else(
                || self.source_buffer.as_ref().map_or(0, |source| source.len()),
                Vec::len,
            )
            .min(u32::MAX as usize) as u32
    }

    pub(crate) fn take_source_text(&mut self) -> Result<String, std::string::FromUtf8Error> {
        if let Some(bytes) = self.source_bytes.take() {
            return String::from_utf8(bytes);
        }
        Ok(self.source_buffer.take().unwrap_or_default())
    }
}
