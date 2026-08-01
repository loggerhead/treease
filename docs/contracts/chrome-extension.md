---
summary: "Chrome extension boundaries, local JSON handling, permissions, and Side Panel behavior."
read_when:
  - Changing the Chrome extension content script, Side Panel, permissions, or privacy behavior
  - Reviewing extension data flow or browser integration boundaries
---

# Chrome Extension Contract

## Product boundary

The extension turns a user click on nearby structured text into a local Graph in
Chrome's Side Panel. It owns browser event handling, bounded DOM extraction,
strict JSON detection, Service Worker routing, Side Panel state, and extension
settings. It does not reimplement Core parsing or Graph semantics.

## Data flow

```text
user click
  → bounded candidate extraction
  → local strict JSON detection
  → Service Worker message
  → Side Panel
  → Core/WASM document job
  → shared normalized graph data
  → shared graph rendering runtime
```

The extension and Web consume the same Core projection normalization path. The
extension must not maintain a weaker copy of node, edge, table, missing-value, or
formatting normalization.

## Candidate extraction

- Handle left-clicks without preventing or delaying the page event.
- Prefer input/textarea value, clicked text, then the nearest bounded `pre`, `code`,
  textarea, or container text.
- Never scan `body`, `html`, or the whole page; stop at the configured ancestor
  depth and reject candidates over the size limit.
- Strip a JSON code fence before strict `JSON.parse`; do not accept JavaScript
  object literals or YAML as JSON.
- Do not process browser-internal pages, extension pages, passwords, cookies,
  form submissions, or network responses.

## Side Panel and permissions

The Side Panel shows Graph, loading, empty, invalid, and oversized states and may
open the full Treease Web entry point. Automatic opening is best-effort because
Chrome user-gesture rules may not survive asynchronous message delivery; failure
must not break the page and may leave a short-lived in-memory result for the user
to open manually.

The manifest permissions must be limited to the Side Panel, settings storage, and
the authorized web pages needed for the click listener. The current MVP handles
top-level pages; it does not promise iframe or Shadow DOM support.

## Privacy and retention

Captured text, parsed JSON, and graph data remain local to the extension process.
The extension does not upload page content, persist full URLs, read cookies or
credentials, or build a browsing-history index. Settings storage contains only
extension preferences and site enable/disable state. Captured content is released
when the panel/tab context is replaced, closed, or expires.

## Runtime boundary

- Core/WASM is the sole parser and document computation implementation.
- Web owns the canonical projection normalization module shared with the extension.
- `packages/graph-viewer-runtime` owns reusable graph rendering/interaction only;
  it does not own snapshots, document authority, or browser permissions.
- Extension modules own only browser adapters and local UI state.
