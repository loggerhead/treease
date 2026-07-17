---
summary: "Product user stories and value goals for aligning experience priorities."
read_when:
  - Understanding feature value, priority, or user journeys
  - Evaluating whether an interaction is worth retaining
---

# User Stories

This document defines the product perspective: who Treease serves, what users' most essential tasks are, and which experience priorities should outweigh local implementation convenience.

## Scope

This document describes only user behavior, goals, and expected experiences. It aligns why users use Treease, how they use it, and the value each capability provides.

## Product Overview

Treease is an editor and Graph workspace for structured text. Users can import structured content into the editor, view and edit text on the left, understand its structure through the Graph on the right, and navigate, edit, compare, and export between the two.

Treease focuses on a continuous experience: users open content, understand its structure, locate fields, edit content, confirm changes, compare differences, and export results. The Graph is not a standalone display, and the editor is not an isolated text box; both serve the same content.

## Target Users

- Configuration and data-file maintainers: need to open, organize, convert, and export structured text such as JSON, YAML, TOML, CSV, Python dicts, and JavaScript objects.
- Frontend and backend developers: need to quickly locate deeply nested fields between the editor, Graph, and Tree Path, and confirm structural changes after edits.
- Debugging and testing users: need to compare two pieces of structured content, copy structural paths, locate differences, and reproduce issues.
- Large-file analysis users: need to import large JSON files and see clear processing progress while waiting.
- Command-line users: need to process structured input in the terminal, control output formats, or update files directly.

## Core User Journeys

1. Import a local file; the system identifies its format, opens it in the editor on the left, and generates a Graph view on the right.
2. Move the cursor to a JSON block in text (a JSONL line or JSON embedded in a log); the Graph automatically extracts and displays that block's structure.
3. Run format, minify, or sort in the editor, or quickly trigger text operations through command search.
4. Search or click nodes in the Graph, synchronize Tree Path, and navigate back to the corresponding editor text range.
5. Hover over a value node in the editor to preview URLs, images, colors, dates, Base64, JWT, Unicode, and more.
6. Open nested subgraph workspaces in the Graph to continue reading, locating, and editing local content along a nested structure.
7. Edit a value in the Graph and write the change back to the editor. Edit in the editor and update the Graph incrementally.
8. Export to a target format, preview the converted result on the right, then download the file.
9. Load a second item into Compare and compare structures first; show textual differences when structural comparison is unavailable.
10. Open a specified editor, viewer, or compare scenario through a URL preset to reproduce a demo, shared link, or issue state.
11. Open Settings to adjust editor, formatting, view, and interaction preferences and save them locally.
12. Use `treease` on the command line to process input, inspect output, or update files directly.

## User Stories

### US-01 Import a File and Identify Its Format Automatically

As a configuration or data-file maintainer, I want a locally selected or dropped file to be identified and loaded into the editor automatically, so I can start viewing and editing without copying its content manually.

- User behavior: Select or drop a local file.
- User expectation: Treease identifies the content format automatically, shows the source text on the left, and displays an understandable structural view on the right.
- User value: Reduces preparation before import and lets users begin reading and editing sooner.
- Expected experience:
  - Supported files open directly in the editor.
  - For content with a displayable structure, the Graph reflects the current text.
  - Users can clearly tell when a file has loaded successfully.

### US-02 Locate and View JSON Blocks in Mixed Text

As a log-investigation or data-debugging user, I want the Graph to automatically parse and display the structure of a JSON block when I move the editor cursor to a JSONL record or JSON fragment in a log, so I can quickly understand the data structure at the current location in mixed content.

- User behavior: Move the editor cursor to a JSON object or array in text, such as a JSONL line or JSON embedded in a log.
- User expectation: Treease extracts the complete JSON container from the cursor position and displays its structure in the Graph.
- User value: Users do not need to manually copy JSON from logs or mixed text and paste it for parsing; the JSON structure at the cursor is directly visible in the Graph.
- Expected experience:
  - When the cursor moves to a JSONL line, the Graph shows that line's JSON structure and not the other lines.
  - When the cursor moves to a JSON fragment embedded in log text, the Graph shows only the JSON portion.
  - If the editor cannot select a valid JSON container, the Graph preserves its previous content.
  - When the cursor leaves the current JSON block, Graph content is replaced or cleared.

### US-03 Organize Text in the Editor

As a structured-text editing user, I want to quickly run format, minify, and sort in the editor, so I can organize content into a form better suited for reading, transfer, or committing.

- User behavior: Click an action in the bottom bar or trigger a text operation through command search.
- User expectation: The current text updates immediately to the corresponding result.
- User value: No need to leave the current page or copy content into an external tool.
- Expected experience:
  - Content is easier to read after formatting.
  - Content is more compact after minification.
  - Field order is more stable after sorting.
  - Actions are easy to discover and can also be invoked quickly through search.

### US-04 Locate Fields in the Graph and Tree Path

As a developer debugging deeply nested structures, I want to search or click nodes in the Graph, or click Tree Path segments, to synchronize the editor selection and quickly find the field's location in the source text.

- User behavior: Search or click nodes in the Graph, inspect the selected node, click Tree Path segments to navigate to their positions, and copy a complete Tree Path for communication.
- User expectation: Tree Path shows the current location, the editor automatically navigates to the matching text range, the Graph highlights the corresponding cell, and clicking a path segment synchronizes the editor and Graph navigation.
- User value: Reduces the cost of understanding deeply nested structures and the time spent manually finding fields, making issue diagnosis and team communication more precise.
- Expected experience:
  - After clicking a Graph node, users see the matching Tree Path and the relevant editor text is selected.
  - Selecting a Graph search result navigates the editor and Graph to the corresponding node.
  - Clicking a parent path segment in Tree Path navigates the editor and Graph to the corresponding node.

### US-05 Preview Values by Hovering in the Editor

As a user reading data, I want to see previews for images, URLs, dates, colors, Base64, JWT, Unicode, and more when hovering over value nodes in the editor, so I can make fewer trips to external tools.

- User behavior: Hover the pointer over a value in the editor.
- User expectation: If the value has interpretable content, Treease displays a preview directly.
- User value: Users can understand the value's meaning in the context of the source text.
- Expected experience:
  - URLs, images, colors, dates, and similar values are recognized intuitively.
  - Previews do not interrupt editing or reading.
  - Values unsuitable for preview do not force distracting information onto users.

### US-06 Click the Graph to Open a Local Workspace

As a user reading a complex Graph, I want clicking a particular cell to open a local workspace at the bottom, so I can keep reading, locating, and editing along the same path instead of relying on transient hover previews.

- User behavior: Click a key, value, index, or row in the Graph.
- User expectation: Treease preserves the reveal/editor linkage and expands a persistent pane at the bottom; objects and arrays use a graph pane, while scalars use a Monaco content pane.
- User value: Users can retain the global structural view while inspecting and editing local details deeply.
- Expected experience:
  - The workspace appears immediately after clicking, without an extra button or hover delay.
  - Clicking a key opens its key-value pair, clicking an index or row opens its row, and clicking a value opens its path.
  - The content pane does not show an additional key input; it shows the path and value editor directly.
  - Value editing in the content pane reuses the existing bidirectional-editing path rather than introducing a second commit semantic.
  - While a user types continuously in a content pane, the workspace does not feed an old external value back and interrupt the current input.

### US-07 Open Nested Subgraph Workspaces in the Graph

As a user reading deeply nested structures, I want clicking a structured cell in the main graph to open a persistent subgraph workspace at the bottom, and to be able to open the next subgraph from it, so I can keep reading and locating content along a nested path without returning to the entire main graph.

- User behavior: Click an object or array value cell in the main graph, then click the next structured cell in the opened subgraph pane.
- User expectation: The bottom workspace expands subgraph panes layer by layer along the current click path; each pane retains the main graph's reading mode, dragging behavior, and default zoom.
- User value: Users can focus on the current nested chain and descend through complex structures without repeatedly zooming around the main graph to find local regions.
- Expected experience:
  - Clicking a structured cell in the main graph immediately displays its subgraph pane at the bottom.
  - Clicking the next structured cell in a subgraph pane expands a new pane on the right. The viewport shows at most three columns at once, preserving the full chain through horizontal scrolling beyond that.
  - The pane title tells users which path they are viewing without consuming extra path-rail space.
  - Each pane uses the same graph-canvas interaction as the main graph, keeps the default size, and supports the same dragging, zooming, and panning-boundary constraints.
  - When users locate a value in a subgraph, the editor remains synchronized to its matching position, keeping the graph-text linkage intact.
  - When users repeatedly edit the same path in a pane, the final save uses the newest input rather than an older intermediate commit.

### US-08 Synchronize Changes Between the Graph and Editor

As a visual-editing user, I want to change values directly in the Graph and synchronize them to the editor. As a text-editing user, I want the Graph to update after I edit in the editor, so both editing modes always work on the same content.

- User behavior: Change a value in the Graph or edit text in the editor.
- User expectation: The other side reflects the change just made.
- User value: Users can choose the more natural editing mode without worrying that the two sides will diverge.
- Expected experience:
  - After a Graph value changes, the editor text changes in sync.
  - After a local edit in the editor, the Graph reflects the new structure.
  - If an edit cannot be completed, users receive clear feedback.

### US-09 Export to a Target Format

As a user migrating configuration between systems, I want to export current content to a target format and preview the result before downloading, so I can confirm that the conversion meets expectations.

- User behavior: Select a target format, inspect the preview, and download the file.
- User expectation: Treease generates target-format text from the current content.
- User value: Users can view, adjust, convert, and export in one workflow.
- Expected experience:
  - The preview helps users confirm the result.
  - The downloaded filename and format match the user's selection.
  - If conversion is unavailable, users receive clear feedback.

### US-10 Compare Two Pieces of Content

As a configuration-review user, I want to load another item into Compare and see structured differences first, so I can ignore irrelevant formatting changes and focus on real content differences.

- User behavior: Open Compare, load or paste another item, and start comparison.
- User expectation: Treease tells users whether the two items are the same and where they differ.
- User value: Configuration review, change confirmation, and issue diagnosis become more efficient.
- Expected experience:
  - Users receive clear equality feedback when the content is the same.
  - Differences are clearly marked when content differs.
  - Textual differences remain visible when structural comparison is unsuitable.

### US-11 Open a Reproducible Entry Point Through a URL Preset

As a user who needs to share an example, demo steps, or issue reproduction, I want a URL preset to directly open a specified editor or viewer state, initial text, and command scenario, so recipients enter the same working state as I do.

- User behavior: Open an `/editor` link with query parameters, or compose an entry link using parameters such as `ui`, `lang`, `text`, `textUrl`, `rightText`, `rightTextUrl`, `command`, `yq`, `nest`, and `autoFormat`.
- User expectation: Treease restores the matching UI visibility, text content, viewer mode, and initial actions from the parameters, and provides understandable ignore rules for parameter conflicts.
- User value: Users can encode how to enter a scenario in the link itself instead of relying on spoken instructions or manually repeating actions.
- Expected experience:
  - Viewer-only, editor-only, compare, text-preview, and similar states can be reproduced directly from a link.
  - The precedence of `text` / `textUrl`, `rightText` / `rightTextUrl`, and `command` / `yq` is clear and predictable.
  - Same-language examples, a shared set of UI-panel visibility choices, and parser/formatting preferences can be restored with the entry point.
  - Users receive a clear failure or ignore notice when parameters are invalid or a resource fails to load.

### US-12 Adjust Personal Preferences

As an advanced user, I want to open Settings, adjust editor, formatting, view, and interaction preferences, and save them locally so Treease better fits my workflow.

- User behavior: Open Settings, change settings, then save or reset them.
- User expectation: Configuration changes affect subsequent use and persist when opened again.
- User value: Advanced users can tailor Treease to their own workflows.
- Expected experience:
  - Settings are easy to inspect and modify.
  - Invalid settings are clearly indicated.
  - Saved settings remain effective after refreshing the page.
  - Resetting restores the default experience.

### US-13 See Progress While Processing Large Files

As a large-file analysis user, I want to see Graph construction progress when importing large JSON files, so I know Treease is processing rather than frozen.

- User behavior: Import a large structured file and wait for the Graph to appear.
- User expectation: Processing provides clear progress feedback.
- User value: Reduces uncertainty while waiting and tells users whether they should continue waiting.
- Expected experience:
  - The Graph renders in batches, with nodes appearing gradually during processing.
  - Progress or status changes are visible while handling a large file.
  - The Graph displays normally after content is ready.
  - If processing fails, users receive clear failure feedback.

### US-14 Process Structured Input on the Command Line

As a command-line user, I want to use `treease` to process input, control the output format, or update files directly, so I can integrate Treease into scripts and everyday terminal workflows.

- User behavior: Run a command in the terminal, pass a file or input content, inspect the output, or write back to a file.
- User expectation: Command behavior is clear, results are predictable, and errors are understandable.
- User value: Users can complete batch processing and everyday automation without opening the Web application.
- Expected experience:
  - Available commands are understandable from help output.
  - Standard processing results are written to the terminal.
  - When users request a file update, its contents are updated.
  - Errors produce clear error messages.

## Product Boundaries

- Treease is for structured text, not a general rich-text editor.
- The Graph helps users understand and locate structure; it does not replace every text-editing scenario.
- Graph editing is better suited to explicit value changes than arbitrary structural adjustments.
- Compare primarily serves structured-content review. Users should still be able to see text-level differences when content cannot be understood structurally.
- Settings targets advanced users familiar with JSON configuration, not a form wizard for every option.
- Command-line capabilities serve automation and batch processing; they are not equivalent to the full interactive Web experience.

## Maintenance Rules

- When adding a user-visible capability, first add the corresponding user story, then decide whether a more detailed workflow document is necessary.
- User stories describe only user goals, behavior, value, and expected experience.
- Do not put content in this document if it cannot be understood from the user's perspective.
