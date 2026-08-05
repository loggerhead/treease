---
summary: "Editor Tab topology, Monaco model ownership, and session restoration invariants."
read_when:
  - Changing editor Tab creation, activation, closing, or Monaco model lifetime
  - Changing workspace session restoration, Tab state ownership, or sidecar lifetime
---

# Editor Tab Lifecycle Contract

## Scope

This contract defines the current Web editor Tab topology, Monaco model ownership,
session restoration, and left-Tab / right-sidecar lifetime. It describes stable
runtime rules, not an implementation sequence.

## Authority and ownership

- `EditorWorkspaceState`, held by `ActiveDocumentAuthority`, is the only authority
  for left-Tab identity, order, names, active/primary selection, pane ownership,
  revision, snapshot binding, text mirror, dirty metadata, and Tab-local UI state.
- A private `EditorCore` runtime owns resident Monaco models in `Map<tabId,
  ITextModel>`. It creates a model from the workspace mirror when needed, installs
  the selected model, and disposes removed models. It does not own Tab topology or
  selection policy.
- `Editor Model` is the draft-text authority. `EditorWorkspaceTab.sourceText` is a
  mirror for inactive/unmounted models, persistence, and visible bindings; it is
  not a competing text authority.
- TopBar, keyboard commands, and host commands call EditorCore Tab commands. Their
  Tab summaries and active id are derived from `editorWorkspace`.
- `WorkspaceHost` is the only session I/O seam for Browser and Desktop. Shared
  editor code does not call IndexedDB or Tauri directly.

## Topology invariants

At every observable point, the header, active/primary Tab, installed Monaco model,
`editorIO`, Graph, and Document Runtime refer to the same `documentKey`.

Main Tabs and sidecars are entities in one `TabId` space. Their explicit
bidirectional relationship fields, never array position or id format, form a tab pair.
`role` controls only left-tab visibility and pane projection. A sidecar is not
placed in left `tabOrder` or serialized as a left Tab, but it is created and
destroyed with its linked main Tab.

`Column Detail Draft` is a separate private Monaco backing entity for the
Column Navigator. Its `column-detail-draft` role is neither a left Tab nor a
paired sidecar, and it never participates in Tab-pair transitions.

Tab transitions are pure workspace transformations. They do not call Svelte,
Monaco, Worker, or Document Runtime and return a closed effect shape:

```ts
type TabTopologyEffect =
  | { kind: 'activate-existing'; tabId: string; disposeTabId?: string }
  | { kind: 'activate-new-blank'; tabId: string; documentKey: string; disposeTabId: string };
```

The transition never uses a null, empty string, fallback Tab, or sentinel id to
represent a missing successor.

There is no runtime `ensure` or rebind operation for sidecars. A production
sidecar is introduced only by pair creation or restore, receives a new document
identity only through main-document replacement, and is removed only by pair
closure. Legacy fixture/session adaptation must validate and normalize topology
before publication; it may not attach an existing sidecar to a different main.

## Transitions

- **Create** generates a new `tabId` and `documentKey`, adds and activates the Tab,
  creates/selects its model, publishes the workspace, then starts the normal
  whole-document commit path.
- **Activate** rejects a missing Tab or sidecar id, updates active/primary/pane
  selection atomically, selects or creates the target model from its mirror, then
  restores document work with `documentKey + revision` freshness.
- **Close inactive** removes the requested main Tab and its linked sidecar,
  invalidates both entities' runtime bindings and operations, leaves the active
  model unchanged, publishes the workspace, then disposes the removed model.
- **Close active with a successor** computes the successor before disposal,
  installs its model first, publishes the new authority state, invalidates the
  closed Tab's operations, and only then disposes the old model.
- **Close the last left Tab** removes its entire pair and creates a fresh blank
  main/sidecar pair with new identities. The blank text is committed through the normal
  `Commit Transaction` so Document Runtime owns clear semantics. No example text
  or closed-document identity is reused.

## Session restoration

Session restoration is a bootstrap operation. `WorkspaceHost.loadSession()` only
provides persisted data; before accepting Tab commands, the editor validates it,
constructs the complete topology, creates/selects the initial model, publishes a
ready authority state, and only then enables commands and persistence.

Browser persistence stores only the workspace draft (Tab name, language, text, and
current Tab) in the browser-owned `treease-workspace` IndexedDB. It does not upload
or restore file-system grants. `/editor?reset=1` clears same-origin IndexedDB
databases except `treease-usage`, local/session storage, removable cookies, and
editor settings/layout. Desktop uses the same WorkspaceHost seam.

## Boundaries

- Tab lifecycle code does not create snapshots, interpret parse results, or write
  Graph state; those semantics belong to Document Runtime.
- All asynchronous visible work uses the operation lifecycle and freshness rules
  in [Editor Operation Lifecycle](./editor-operation-lifecycle.md).
- No second workspace store, Tab list, active id, or synchronization facade may be
  introduced.
- Async work captures one target `{ tabId, documentKey, generation, revision }`.
  A hidden target may complete and update its own tab-local state; only a visible
  target may project to Monaco, Graph DOM, focus, scroll, or notifications.
