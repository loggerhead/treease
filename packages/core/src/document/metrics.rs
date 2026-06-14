use std::sync::{LazyLock, Mutex};

use super::protocol::EventBatch;

static GLOBAL_DOCUMENT_ENGINE_METRICS: LazyLock<Mutex<DocumentEngineMetrics>> =
    LazyLock::new(|| Mutex::new(DocumentEngineMetrics::default()));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DocumentEngineMetrics {
    pub jobs_started: u64,
    pub jobs_advanced: u64,
    pub jobs_cancelled: u64,
    pub terminal_batches: u64,
    pub commit_incremental_fallback_full_parses: u64,
}

impl DocumentEngineMetrics {
    pub fn record_job_started(&mut self) {
        self.jobs_started = self.jobs_started.saturating_add(1);
    }

    pub fn record_job_advanced(&mut self, batch: &EventBatch) {
        self.jobs_advanced = self.jobs_advanced.saturating_add(1);
        if batch.terminal.is_some() {
            self.terminal_batches = self.terminal_batches.saturating_add(1);
        }
    }

    pub fn record_job_cancelled(&mut self, batch: &EventBatch) {
        self.jobs_cancelled = self.jobs_cancelled.saturating_add(1);
        if batch.terminal.is_some() {
            self.terminal_batches = self.terminal_batches.saturating_add(1);
        }
    }

    pub fn record_job_closed(&mut self, batch: &EventBatch) {
        if batch.terminal.is_some() {
            self.terminal_batches = self.terminal_batches.saturating_add(1);
        }
    }

    pub fn record_commit_incremental_fallback_full_parse(&mut self) {
        self.commit_incremental_fallback_full_parses = self
            .commit_incremental_fallback_full_parses
            .saturating_add(1);
    }
}

pub(crate) fn with_global_document_engine_metrics<R>(
    f: impl FnOnce(&mut DocumentEngineMetrics) -> R,
) -> Option<R> {
    let mut metrics = GLOBAL_DOCUMENT_ENGINE_METRICS.lock().ok()?;
    Some(f(&mut metrics))
}

pub fn global_document_engine_metrics_snapshot_for_tests() -> DocumentEngineMetrics {
    GLOBAL_DOCUMENT_ENGINE_METRICS
        .lock()
        .map(|metrics| *metrics)
        .unwrap_or_default()
}

pub fn reset_global_document_engine_metrics_for_tests() {
    if let Ok(mut metrics) = GLOBAL_DOCUMENT_ENGINE_METRICS.lock() {
        *metrics = DocumentEngineMetrics::default();
    }
}
