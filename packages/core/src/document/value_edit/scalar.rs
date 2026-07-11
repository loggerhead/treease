use crate::tree::{DocumentTextEdit, TreeNode, TreeNodeKind};
use crate::wasm_types::{PathSeg, PathSpan};

use super::{
    GraphValueEditContext, GraphValueEditPlanner, edit_value_as_scalar_string,
    edit_value_to_plain_json, graph_value_edit_edits, graph_value_edit_fallback,
    request_path_segments, span_to_edit,
};
use crate::document::protocol::{GraphValueEditFallbackReason, GraphValueEditPlan};

pub(super) trait ScalarGraphValueEditRules {
    fn supports_key_edit(&self) -> bool {
        true
    }

    fn normalize_span(&self, _source: &str, _prefer_key: bool, span: PathSpan) -> Option<PathSpan> {
        normalize_default_scalar_graph_edit_span(span)
    }

    fn recover_span(
        &self,
        _source: &str,
        _path: &[PathSeg<'_>],
        _prefer_key: bool,
    ) -> Option<PathSpan> {
        None
    }

    fn build_missing_target_edit(
        &self,
        _source: &str,
        _path: &[PathSeg<'_>],
        _prefer_key: bool,
        _replacement: &str,
    ) -> Option<DocumentTextEdit> {
        None
    }

    fn format_subtree_value(&self, _value: &serde_json::Value) -> Option<String> {
        None
    }

    fn format_key(&self, key: &str) -> Option<String>;

    fn format_value(&self, value: &serde_json::Value, target: Option<&TreeNode>) -> Option<String>;
}

pub(super) struct ScalarGraphValueEditPlanner<R: ScalarGraphValueEditRules> {
    rules: R,
}

impl<R: ScalarGraphValueEditRules> ScalarGraphValueEditPlanner<R> {
    pub(super) const fn new(rules: R) -> Self {
        Self { rules }
    }
}

impl<R: ScalarGraphValueEditRules + Sync> GraphValueEditPlanner for ScalarGraphValueEditPlanner<R> {
    fn plan(&self, ctx: GraphValueEditContext<'_>) -> GraphValueEditPlan {
        let path = request_path_segments(&ctx.request.path);
        let recovered_span =
            self.rules
                .recover_span(&ctx.analysis.source, &path, ctx.request.prefer_key);
        let target = crate::tree::find_node_by_path_with_index(
            ctx.document.root,
            &path,
            ctx.request.prefer_key,
            &ctx.document.store,
            ctx.path_index,
        )
        .and_then(|target_id| ctx.document.store.get(target_id));
        if let Some(target) = target {
            if target.kind != TreeNodeKind::Scalar {
                if let Some(formatted) = self
                    .rules
                    .format_subtree_value(&edit_value_to_plain_json(&ctx.request.value))
                {
                    let span = self.resolve_span(ctx, &path);
                    return span
                        .and_then(|span| span_to_edit(span, formatted))
                        .map(|edit| graph_value_edit_edits(vec![edit]))
                        .unwrap_or_else(|| {
                            graph_value_edit_fallback(GraphValueEditFallbackReason::InvalidPath)
                        });
                }
                return graph_value_edit_fallback(GraphValueEditFallbackReason::UnsupportedEdit);
            }
        } else if recovered_span.is_none() {
            return graph_value_edit_fallback(GraphValueEditFallbackReason::InvalidPath);
        }
        if ctx.request.prefer_key && !self.rules.supports_key_edit() {
            return graph_value_edit_fallback(GraphValueEditFallbackReason::UnsupportedEdit);
        }
        if !ctx.request.prefer_key && !is_plain_scalar_edit_value(&ctx.request.value) {
            return graph_value_edit_fallback(GraphValueEditFallbackReason::UnsupportedEdit);
        }

        let Some(replacement) =
            self.format_replacement(ctx.request.prefer_key, &ctx.request.value, target)
        else {
            return graph_value_edit_fallback(GraphValueEditFallbackReason::InvalidReplacement);
        };

        if target.is_none() {
            return self
                .rules
                .build_missing_target_edit(
                    &ctx.analysis.source,
                    &path,
                    ctx.request.prefer_key,
                    &replacement,
                )
                .map(|edit| graph_value_edit_edits(vec![edit]))
                .unwrap_or_else(|| {
                    graph_value_edit_fallback(GraphValueEditFallbackReason::InvalidPath)
                });
        }

        let Some(span) = self.resolve_span(ctx, &path) else {
            return graph_value_edit_fallback(GraphValueEditFallbackReason::InvalidPath);
        };

        span_to_edit(span, replacement)
            .map(|edit| graph_value_edit_edits(vec![edit]))
            .unwrap_or_else(|| graph_value_edit_fallback(GraphValueEditFallbackReason::InvalidPath))
    }
}

impl<R: ScalarGraphValueEditRules> ScalarGraphValueEditPlanner<R> {
    fn format_replacement(
        &self,
        prefer_key: bool,
        value: &serde_json::Value,
        target: Option<&TreeNode>,
    ) -> Option<String> {
        let plain = edit_value_to_plain_json(value);
        if prefer_key {
            let key = edit_value_as_scalar_string(&plain);
            return self.rules.format_key(&key);
        }
        self.rules.format_value(&plain, target)
    }

    fn resolve_span(
        &self,
        ctx: GraphValueEditContext<'_>,
        path: &[PathSeg<'_>],
    ) -> Option<PathSpan> {
        let span = crate::tree::compute_path_span_for_document_with_index(
            &ctx.document.store,
            ctx.document.root,
            ctx.analysis.ts_tree.as_ref(),
            &ctx.analysis.diagnostics,
            &ctx.analysis.language,
            &ctx.analysis.source,
            path,
            ctx.request.prefer_key,
            ctx.path_index,
        );
        if let Some(span) =
            self.rules
                .normalize_span(&ctx.analysis.source, ctx.request.prefer_key, span)
        {
            return Some(span);
        }
        self.rules
            .recover_span(&ctx.analysis.source, path, ctx.request.prefer_key)
    }
}

pub(super) fn normalize_default_scalar_graph_edit_span(span: PathSpan) -> Option<PathSpan> {
    (span.start_byte >= 0 && span.end_byte > span.start_byte).then_some(span)
}

fn is_plain_scalar_edit_value(value: &serde_json::Value) -> bool {
    matches!(
        edit_value_to_plain_json(value),
        serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_)
    )
}
