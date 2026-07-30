---
summary: "Column Navigator: JSON Hero-style path-driven column navigation for the Treease document."
read_when:
  - The task involves the JSON Hero-style subgraph workspace.
  - The task involves column navigation, leaf detail editing, or workspace keyboard navigation.
---

# Column Navigator

`Column Navigator`（中文：`列导航器`）是这块底部功能区的统一产品名称。
右侧编辑区域统一称为 `Column Detail Editor`（中文：`列详情编辑器`）。本文中的
`subgraph workspace` 仅表示历史生命周期和数据域术语；列轨道、路径导航和详情编辑器
均属于 Column Navigator，不再称为 graph pane chain 或 nested panes。

## Goal

Replace the current subgraph workspace's graph-pane chain with the `Column Navigator`, a path-driven column browser modeled on JSON Hero. The algorithm and interaction model should follow JSON Hero's Column View, while the visual language, theme, typography, spacing, and components remain Treease-native.

The main Treease graph and editor remain unchanged. This document defines the bottom subgraph workspace only.

## Scope

Implement the green-framed area from the product reference:

- Path navigation.
- Path-driven columns.
- Horizontal browsing across columns.
- The right-side detail editor.
- Selection, history, and keyboard navigation within the workspace.

Do not implement the red-framed JSON Hero toolbar:

- JSON Hero's edit button.
- JSON Hero's search input and search entry point.
- Other JSON Hero-specific top-toolbar actions.

Existing Treease search, editing, and top-level controls remain governed by their existing contracts.

## Core Model: Path-driven Columns

The workspace is a projection of one active tree path. Each column represents one object or array on that path and displays its direct children.

For a path such as:

```text
root → agent_steps → index 0 → basic_info
```

the workspace renders a corresponding sequence of columns from left to right.

### Navigation rules

- Selecting an object or array creates the next column to its right.
- Selecting a sibling keeps the ancestor columns and replaces all descendant columns.
- The active path is the source of truth for which columns exist.
- Columns are not independent graph panes and do not maintain an unrelated pane chain.
- The workspace must preserve the full path chain; it must not truncate or compress the chain to a fixed number of visible panes.
- Empty objects and arrays are still containers in the column model, but because they have no children they resolve to the detail editor rather than producing an empty navigation column.
- Missing placeholder cells cannot open another column.

## Default Expansion

Opening a container defaults to two levels of column expansion:

1. The current container's column.
2. The next container column on the active path, when one exists.

If the next selected node is a leaf, or there is no next container to render, the workspace does not create another column. It opens the right-side detail editor instead.

Default expansion must respect the current path and snapshot. It must not invent a sibling selection or mutate document data merely to fill the second column.

## Leaf Nodes and Right-side Detail Editor

Leaf nodes include strings, numbers, booleans, `null`, and other non-object/non-array values. Selecting a leaf opens the right-side detail area directly; no leaf column is created.

When the selected node is a non-empty object or array, the workspace keeps its
child column open and also binds the right-side detail area to that same path.
The detail editor displays the complete subtree text returned by the active
snapshot projection, so the selected container can be inspected without
leaving the column path.

The `Column Detail Editor` uses Monaco Editor and follows the current subgraph workspace editing contract:

- It uses the same Monaco-based content-pane logic as the existing subgraph workspace.
- It displays the value at the selected path, not a new independent document.
- It supports bidirectional editing: Monaco edits update the structured document, and document changes update the editor.
- It uses the existing local draft, pending-commit, freshness, and revision semantics.
- It edits the value by default and does not add a separate key editor.
- It must handle commit failures, stale asynchronous results, and refreshes using the existing subgraph workspace behavior.
- Switching selection must preserve or resolve the current draft according to the existing editing contract before binding the editor to another path.

The detail editor is Treease-styled. JSON Hero supplies the interaction concept only; it does not supply colors, fonts, Monaco configuration, or visual components.

## Scrolling and Layout

The column rail follows JSON Hero's native scrolling behavior:

- The workspace uses one native horizontal scrolling container.
- Each column has a fixed width and does not shrink.
- The column rail uses horizontal overflow rather than Leafer canvas translation.
- Each column owns its vertical scrolling area.
- Horizontal scrolling between columns and vertical scrolling within a column are independent.
- Mouse-wheel input follows native browser scrolling and must not be converted into workspace pan or zoom.
- When selection or path changes, the active item is scrolled into view using nearest-block behavior and centered horizontally when appropriate.

The graph canvas pan/zoom model is not used for the column rail. The workspace is a data browser; it is not a collection of independently movable graph canvases.

## Interaction Rules

The interaction model follows JSON Hero's Column View:

- Mouse click selects an item.
- Up and Down move between siblings.
- Right moves into the selected container's children.
- Left moves to the parent.
- `Esc` resets a temporary selection state where applicable.
- Back and Forward navigate workspace history.
- The path navigation allows returning to an ancestor path.
- Selected and highlighted nodes remain distinct when the interaction state requires it.
- Path changes reconcile the visible columns and the right-side detail editor from one active path state.

### Keyboard focus boundary

Keyboard navigation is enabled only while focus is inside the subgraph workspace.

- The workspace root must be focusable.
- Focusing any interactive element inside the workspace counts as workspace focus.
- Keyboard handlers must be scoped to the workspace focus boundary rather than installed as global document/window handlers.
- Arrow keys, `Esc`, history shortcuts, and copy/navigation shortcuts must not intercept input while focus is in the main editor, Monaco detail editor, search controls, top bar, or any other area outside the workspace.
- When Monaco has text-editing focus, normal Monaco cursor and selection behavior takes precedence over workspace navigation. Workspace-level navigation may resume after focus returns to the workspace navigation surface.
- Moving focus outside the workspace immediately disables workspace keyboard navigation.

## Treease Integration

The replacement changes the subgraph workspace presentation and navigation model, not Treease's document authority:

- Core parsing, paths, snapshots, and projections remain the source of data.
- The main graph remains the primary graph view.
- The workspace remains a persistent bottom workspace, not a hover preview or a second document.
- Treease's existing bidirectional-edit semantics remain authoritative for the right-side Monaco editor.
- Workspace state must continue to refresh or invalidate when the active snapshot, revision, render configuration, or nesting configuration changes.
- Closing or removing the workspace releases temporary editor and projection state.
- Whole-document replacement, import, language switch, and initial-example reconstruction reset the workspace according to the existing lifecycle contract.

## State Requirements

The workspace state must have one canonical active path and derive the following from it:

- Visible columns.
- Selected node.
- Highlighted node.
- Right-side leaf detail target, if any.
- Back/forward history.
- Scroll-to-selection requests.

Column rendering must not maintain a second, conflicting navigation state. Selecting an item, restoring a path, keyboard navigation, and opening a leaf editor must all transition through the same path-driven state model.

## Acceptance Criteria

- A container opens as a JSON Hero-style column path, not as a Leafer graph pane.
- The initial state expands two levels when the data allows it.
- A leaf opens the right-side Monaco detail editor without creating a column.
- Editing the leaf in Monaco updates the underlying structured document.
- External document updates appear in the Monaco detail editor.
- Sibling navigation replaces descendant columns while retaining ancestors.
- The complete path remains horizontally browsable with fixed-width columns.
- Column and detail-editor scrolling do not interfere with one another.
- Workspace keyboard navigation works when focus is inside the workspace.
- The same keyboard shortcuts do not affect the main editor or other UI when focus is outside the workspace.
- Monaco cursor movement and text editing continue to work normally while Monaco owns focus.
- The red-framed JSON Hero search/edit toolbar is absent.
- The result uses Treease's existing theme, typography, and visual components.

## Implementation Notes

The current implementation lives in the Web GraphViewer workspace controller
and its DOM view adapter. The controller stores one `activePath`; the column
chain is rebuilt from that path's prefixes whenever navigation or projection
context changes. Selecting a sibling therefore keeps all ancestor columns and
replaces every descendant column by construction.

The rail uses fixed-width, non-shrinking DOM columns inside one native
horizontal scroller. Each column has its own vertical scroller, and selection
changes use native `scrollIntoView({ block: 'nearest' })` behavior. No Leafer
canvas pan/zoom or wheel translation is involved.

The right-side detail editor is the existing Monaco content-pane surface. It
waits for the current path's pending commit before rebinding, queues newer
drafts behind an in-flight commit, and rejects stale projection/commit results
through the existing revision and freshness guards. Clean external document
updates refresh the bound Monaco value; focused or dirty drafts retain the
existing editing contract.

Workspace keyboard handling is scoped to the workspace focus boundary. Arrow
navigation, `Esc`, history, and breadcrumb actions are inactive outside that
boundary, while Monaco keeps its native cursor, selection, and input behavior.
The JSON Hero toolbar, search entry point, and edit button are intentionally not
part of this implementation.

## Verification

The feature is covered by workspace controller/unit tests, DOM workspace tests,
and subgraph integration coverage. The relevant local proof is:

```text
pnpm check
pnpm test:unit
pnpm test:integration
pnpm build:e2e
pnpm exec playwright test test/e2e/subgraph-workspace.spec.ts test/e2e/subgraph-edit-click-highlight-regression.spec.ts --workers=1
pnpm check:circular
```
