---
summary: "Chrome Web Store single-purpose text and reviewer test instructions."
read_when:
  - Submitting the Treease extension to the Chrome Web Store.
---

# Single purpose

Treease identifies JSON in the webpage element a user clicks and visualizes it locally as a graph in Chrome's native Side Panel.

# Reviewer test steps

1. Load the unpacked `apps/extension/dist` directory in Chrome.
2. Click the Treease toolbar icon and acknowledge the local-processing notice.
3. Open a page with a `pre`, `code`, or `textarea` containing valid JSON.
4. Click the JSON. Confirm that the Side Panel opens and shows a graph.
5. Click invalid JSON and an over-1-MB candidate. Confirm the appropriate status when the panel is open.
6. Pause the current site and confirm clicking JSON no longer changes the panel.
7. In browser developer tools, confirm the extension makes no network request containing webpage or JSON content.
8. In a same-origin or cross-origin iframe that Chrome permits the extension to inject into, click JSON and verify the current tab panel updates. Verify a closed Shadow DOM host does not expose its inner text.
9. Click YAML, TOML, and prose containing an embedded strict JSON object. Confirm each valid Core parse renders locally; JavaScript object literals and invalid JSON report an error rather than being accepted as JSON.

# Release gate

The repository's Treease Community License v1.0 permits commercial use and distribution only when the distributor's consolidated annual gross revenue is at most USD 100,000. Above that threshold, prior written authorization from the Licensor is required. A distributed extension must include the same license. Confirm the distributor's revenue status, include `LICENSE` in the packaged distribution, and obtain written authorization before any above-threshold commercial release.
