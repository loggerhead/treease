---
summary: "Guidance for the editor workspace's compact blue-and-white visual system."
read_when:
  - Adding or restyling a workspace-chrome component in apps/web
  - Choosing component spacing, typography, colors, or interaction states
  - Reviewing a change to Sidebar, Function Bar, tabs, or workspace popovers
---

# Workspace UI Style Guide

This guide owns the visual language of the editor workspace chrome: Sidebar,
Function Bar, Graph Top Bar, Tab Switcher, Tree Path Bar, and their popovers.
It is guidance for implementation, not a replacement for the responsibilities
in the [Surface Glossary](contracts/product-surface-glossary.md).

## Direction

Keep the workspace compact, calm, and legible: cool white surfaces, ocean-blue
selection and focus, restrained borders, and small controls that leave room for
the document and graph. Prefer a clear hierarchy over decorative treatment.

- Reuse the tokens in `apps/web/src/app.css`; do not introduce one-off theme
  colors, spacing values, or control treatments for workspace chrome.
- Keep shell layout and visual styling separate. A visual change must not move
  a surface or change its responsibility.
- Editor, Graph, node/edge/highlight, and minimap styling are independent
  systems. Do not change them through workspace-chrome rules without an
  explicit product decision.

## Use the existing scale

- Use the shared spacing rhythm (`--space-*`) for gaps and padding. Smaller
  values belong within a control; larger values separate groups or popover
  sections.
- Use `--topbar-height` for peer workspace rails. Tree Path Bar and Tab
  Switcher are intentionally the same height.
- Use `--control-height` and `--control-radius` for compact actions. New
  toolbar buttons should not invent a larger button merely to look prominent.
- Use the UI typography tokens: 12px for navigation and labels, 13px for
  explanatory body copy, 16px for popover titles, and 10–11px only for compact
  metadata or tooltips.

## States and motion

- Default controls are quiet. Hover adds a light surface change; selected
  controls use the blue accent and soft blue fill; focus uses `--focus-ring`.
- Reuse `--control-transition` for peer controls. Do not add movement,
  bounce, or a unique hover animation to a single button in an existing group.
- Keep popover entry, tooltip, and layout motion short and unobtrusive. Respect
  reduced-motion behavior where the component already provides it.

## Reference components

When adding or changing a component, copy the closest reference's structure
and states before extending it.

| Need | Reference | Follow |
| --- | --- | --- |
| Compact text or icon action | `FunctionBar.svelte` | Height, radius, hover, focus, shared transition, and keyboard-hint treatment. |
| Graph-side action | `GraphTopBar.svelte` | The Function Bar button rules, grouped in a quiet control rail. |
| Sidebar navigation item or toggle | `Item.svelte` and `Sidebar.svelte` | Icon/label spacing, active stripe, compact density, and brand treatment. |
| Sidebar popover | `ContextItem.svelte` and `Sidebar.svelte` | Anchor, 12px base padding, 16px business-dialog padding, and blue focus states. |
| Document tab | `TabSwitcher.svelte` | Compact label sizing, soft selected state, and close/new ghost-action sizing. |
| Bottom-path or navigator action | `TreePathBar.svelte` and `ColumnNavigatorControls.svelte` | Small icon action sizing and shared transition. |
| Form-style workspace popover | `FeedbackDialog.svelte`, `ShareDialog.svelte`, or `SettingsDialog.svelte` | 16px title, 12px labels, 13px body copy, compact inputs, and 28px footer actions. |

## Review checklist

Before merging a workspace UI change, check:

- Does it reuse existing tokens and a reference component instead of adding a
  new local style system?
- Are sibling controls equal in height, typography, radius, and motion?
- Does the added density preserve text readability and visible focus?
- Does it leave the Editor, Graph, and minimap visual systems untouched?
