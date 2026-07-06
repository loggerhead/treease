pub mod diagnostics;
pub mod document_analysis;
pub mod line_index;
pub mod span_index;

pub use diagnostics::{
    DiagnosticLocation, DiagnosticSnippet, DiagnosticStage, Diagnostics, ParseErrorInfo,
    compute_location_and_snippet,
};
pub use document_analysis::{
    DocumentAnalysisDemand, ErrorSpan, StoredDocumentAnalysisDemand, StoredDocumentAnalysisOwned,
    TransientDocumentAnalysis, analyze_document_internal,
    analyze_document_internal_via_streaming_codec, analyze_document_internal_with_demand,
    analyze_document_internal_with_prepared_tree,
    analyze_document_internal_with_prepared_tree_and_demand, collect_error_spans,
    error_spans_to_diagnostics_raw, store_transient_document_analysis,
};
pub use line_index::{LineBounds, LineColumn, LineIndex};
pub use span_index::StructuralSpanIndex;
