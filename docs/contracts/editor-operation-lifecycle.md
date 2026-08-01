---
summary: "Per-Tab document operation ownership, freshness, cancellation, and replacement rules."
read_when:
  - Changing full-edit, import, formatting, or whole-document replacement behavior
  - Changing asynchronous editor results, DocumentJob cancellation, or active UI landing
---

# Editor Operation Lifecycle Contract

## Scope

This contract defines how asynchronous document operations belong to editor Tabs,
how stale results are rejected, and how document and visible UI landing differ.
It covers full-edit, import, conversion, formatting, and programmatic replacement.

## Ownership

Each left Tab has at most one current document operation. A Tab's new operation
supersedes its previous operation; operations on different Tabs are independent.
The Tab operation runtime owns generation, conversion, stream reader, decoder,
RAF, format queue, in-flight Worker request, and `DocumentJob` cancellation. It
does not own Tab order, active selection, persistence, or Graph semantics.

Graph rendering owns only its batch consumer/attachment. Detaching a Graph
attachment never cancels a `DocumentJob`; only same-Tab supersede, Tab close,
Editor disposal, or explicit user cancellation may do so. Every cancellation and
cleanup action is idempotent.

## Stable operation target

Every asynchronous operation captures a target before its first await:

```ts
type EditorDocumentOperationTarget = {
  tabId: string;
  model: Monaco.editor.ITextModel;
  ownerKey: string;
  documentKey: string;
  revision: number;
  languageId: SupportedEditorLanguageId;
  generation: number;
};
```

Callbacks must validate the captured target rather than re-reading the currently
active Tab as a substitute.

## Freshness

An operation is document-current only when its Tab still exists, generation is
unchanged, resident model is the captured non-disposed model, document identity,
revision, language, and cancellation state still match.

Visible freshness additionally requires that the target Tab is active, the editor
still has the captured model, and the active document context has the captured
`documentKey + revision`. Switching Tabs does not make a document operation stale,
but it prevents that operation from changing visible Editor, Graph, diagnostics,
cursor, selection, toast, or preview state.

Stale results may perform only idempotent cleanup. They must not update the target
document, workspace snapshot binding, or visible UI after document freshness fails.

## Canonical write path

All primary-document writes use:

```text
target Tab/model
  → Editor Model
  → Commit Transaction
  → Document Runtime
  → DocumentSnapshot / terminal state
```

Document landing and visible landing are separate. A background Tab may receive
its own canonical source, terminal state, and snapshot binding; only a visible-fresh
result may update the active Graph or other visible projections.

## Whole-document replacement

File replacement, import conversion, format/minify/compact/sort, language switch,
preset, share restore, and other programmatic replacements use one target-aware
replacement boundary:

1. Capture the target model, document identity, revision, language, and generation.
2. Invalidate the old generation synchronously.
3. Apply the target document identity/language/revision transition.
4. Update the target model and workspace mirror.
5. Start the one canonical Commit Transaction.
6. Update visible UI only when visible freshness still holds.

Equal text does not start a replacement. A replacement result from a closed,
rotated, advanced, or superseded target is discarded after cleanup.

## Format and import behavior

The command captures source text, language, options, and target before queuing. A
format result reads none of these from the active Tab at completion. The same Tab
serializes its own commands; a command in one Tab does not block or cancel another
Tab. Cursor reset is visible-only and occurs only after a successful target-current
replacement.

## Share and sidecar boundaries

Share promotion and sidecar state do not own document operations. Share restore and
promotion enter this lifecycle through the target-aware replacement and Commit
Transaction paths. Sidecar operations remain outside the left-Tab operation
registry.

## Verification invariants

- A closed Tab cannot land a late result.
- A background Tab can finish without changing the active Tab's visible state.
- A same-Tab superseding operation invalidates the old generation before cleanup.
- Graph attachment disposal cannot cancel its Tab's DocumentJob.
- Every programmatic whole-document write has exactly one canonical commit entry.
