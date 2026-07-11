use super::job::entry::DocumentJobHandle;
use super::projection::{
    MaterializeBaseContext, MaterializeResult, materialize, materialize_with_base_context,
};
use super::protocol::{DocumentInputPlan, DocumentJobKind, OutputPlan};
use super::runtime::DocumentRuntime;
use super::snapshot::{DocumentSnapshot, GraphProjection};

/// Result classification for one non-streaming `DocumentJob` materialization.
///
/// The module owns how a job becomes a candidate snapshot. `DocumentRuntime`
/// remains responsible for snapshot identity, freshness, commit, and terminal
/// lifecycle semantics.
pub(crate) enum MaterializationOutcome {
    Ready {
        snapshot: DocumentSnapshot,
        output: OutputPlan,
    },
    DiagnosticsOnly {
        snapshot: DocumentSnapshot,
        output: OutputPlan,
    },
    Rejected {
        code: &'static str,
        detail: String,
    },
}

pub(crate) fn materialize_job(
    runtime: &mut DocumentRuntime,
    handle: DocumentJobHandle,
) -> MaterializationOutcome {
    let (spec, submitted_source) = {
        let Some(entry) = runtime.job_mut(handle) else {
            return reject_missing_job(handle);
        };
        let spec = entry.spec.clone();
        let source = match entry.take_source_text() {
            Ok(source) => source,
            Err(error) => {
                return MaterializationOutcome::Rejected {
                    code: "invalid_utf8_source",
                    detail: format!("document source is not valid UTF-8: {error}"),
                };
            }
        };
        (spec, source)
    };

    let result = match spec.kind {
        DocumentJobKind::AnalyzeSource => materialize(
            &DocumentInputPlan::SourceText,
            &spec.document_key,
            &spec.language,
            &submitted_source,
            false,
            &spec.output,
            &[],
            None,
        ),
        DocumentJobKind::ApplyEdits => {
            let Some(base_snapshot_id) = spec.base_snapshot_id else {
                return MaterializationOutcome::Rejected {
                    code: "missing_base_snapshot",
                    detail: "ApplyEdits requires a base snapshot".to_owned(),
                };
            };
            let Some(base) = runtime.snapshot(base_snapshot_id) else {
                return MaterializationOutcome::Rejected {
                    code: "base_snapshot_not_found",
                    detail: "ApplyEdits base snapshot is not available".to_owned(),
                };
            };
            if base.document_key != spec.document_key {
                return MaterializationOutcome::Rejected {
                    code: "base_snapshot_document_mismatch",
                    detail: "ApplyEdits base snapshot belongs to a different document".to_owned(),
                };
            }
            let Some(base_analysis) = base.analysis.as_ref() else {
                return MaterializationOutcome::Rejected {
                    code: "base_snapshot_missing_analysis",
                    detail: "ApplyEdits base snapshot has no analysis".to_owned(),
                };
            };

            materialize_with_base_context(
                &DocumentInputPlan::BaseTextWithEdits,
                &spec.document_key,
                &spec.language,
                &base_analysis.source,
                false,
                &spec.output,
                &spec.edits,
                base_analysis.ts_tree.clone(),
                MaterializeBaseContext {
                    document: base_analysis.document.as_ref(),
                    incremental: base.incremental.as_ref(),
                    line_index: Some(&base_analysis.line_index),
                    semantic_tokens: Some(&base_analysis.semantic_tokens),
                },
            )
        }
    };

    classify_materialization(&spec.document_key, spec.output, result)
}

pub(crate) fn validate_snapshot_ready_outputs(
    graph: Option<&GraphProjection>,
    output: &OutputPlan,
) -> Result<(), String> {
    if !output.graph {
        return Ok(());
    }
    let projection = graph.ok_or_else(|| "requested main graph was not produced".to_owned())?;
    if projection.graph_data.is_none() && !projection.clear {
        return Err("requested main graph was empty".to_owned());
    }
    Ok(())
}

fn classify_materialization(
    document_key: &str,
    output: OutputPlan,
    result: MaterializeResult,
) -> MaterializationOutcome {
    let diagnostics_only =
        result.analysis.document.is_none() && !result.analysis.diagnostics.is_empty();
    if !diagnostics_only {
        if let Err(detail) = validate_snapshot_ready_outputs(result.graph.as_ref(), &output) {
            return MaterializationOutcome::Rejected {
                code: "missing_requested_main_graph",
                detail,
            };
        }
    }

    let mut snapshot = DocumentSnapshot::with_analysis(document_key.to_owned(), result.analysis);
    snapshot.graph = result.graph;
    snapshot.incremental = result.incremental;

    if diagnostics_only {
        MaterializationOutcome::DiagnosticsOnly { snapshot, output }
    } else {
        MaterializationOutcome::Ready { snapshot, output }
    }
}

fn reject_missing_job(handle: DocumentJobHandle) -> MaterializationOutcome {
    MaterializationOutcome::Rejected {
        code: "document_runtime_missing_job",
        detail: format!("No document runtime job registered for handle {}", handle.0),
    }
}
