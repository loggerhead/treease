---
summary: "Code-preview mode, structure generation, and share-resource persistence rules."
read_when:
  - Changing structure generation, read-only code preview, or code-preview sharing
  - Changing share restoration of right-pane modes or generated source
---

# Code Preview and Share Contract

## Scope

Code preview is a right-pane presentation of generated source. It is not a
structured document, does not become a new Editor Model, and does not enter Core
parsing, Graph construction, or primary-document commit state.

## Modes and data

The right pane has explicit modes. `structured` renders the normal structured-data
surface; `code-preview` renders generated source in a read-only editor. A
code-preview resource contains its generated source, target language, root/type
metadata, and the right-pane mode. Structured resources cannot use a code-preview
language, and code-preview resources cannot use a structured-data mode.

Generated source is treated as opaque display text. It is never interpreted as a
new Treease document and is never sent through the Document Runtime.

## Generation flow

Structure generation captures the active document context (Tab, document key,
revision, language, and source) before conversion or generation begins. A
conversion or generation result may enter the right-pane preview only if that
captured context is still current and visible. A stale result is discarded and
cannot replace the current preview or source document.

Supported non-JSON input may first be converted to JSON. Generation failures leave
the source document and existing structured mode unchanged. Successful generation
switches the right pane to read-only `code-preview`.

## Sharing and restoration

Share creation stores the selected code-preview mode, generated source, and target
language in the share resource. It does not rerun generation. Restoring a
code-preview share restores the source document and right-pane preview, skips
structured Graph/Column Navigator restoration, and uses the normal share freshness
rules. Older text-snapshot resources remain readable.

Sharing carries only content the user explicitly selected for sharing. Share
services do not execute AI or deterministic generation during creation or restore.

## Boundaries

- `packages/share-protocol` owns the serialized resource shape.
- `packages/api-contracts` owns HTTP envelopes and errors.
- Web owns right-pane mode and read-only presentation.
- Core owns structured-document parsing and graph semantics, but never code-preview
  interpretation.
- Server code-generation endpoints return generated source; they do not mutate the
  caller's document.

## Stable verification

- Code preview is read-only and does not trigger structured parsing or graph edits.
- A shared code preview restores the same source and language without regeneration.
- A stale generation response cannot replace a newer active preview.
- Invalid mode/language combinations are rejected at the share-protocol boundary.
