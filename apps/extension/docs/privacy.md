---
summary: "Treease Chrome Extension privacy policy and local-data boundary."
read_when:
  - Preparing the Chrome Web Store listing or changing extension data handling.
---

# Treease Chrome Extension privacy policy

Treease processes only text near a webpage element that the user explicitly clicks. It first recognizes strict JSON with `JSON.parse`; it can also send explicitly clicked YAML, TOML, or an embedded strict JSON block to the local Treease Core parser to render a graph in the Chrome Side Panel.

The extension does not upload webpage text, JSON, parsed data, URLs, click records, cookies, passwords, browsing history, or network responses. It does not read password fields, editable form content, cookies, local storage, or form submission data. A user-clicked `input` or `textarea` can be read locally because that is an explicit extraction path. It observes open Shadow DOM only through the browser event's composed path; closed Shadow DOM is not bypassed. It runs independently in frames Chrome injects into and never reads across a frame boundary.

Valid JSON is retained only in short-lived memory scoped to the current tab. It is released on navigation, tab closure, timeout, or Service Worker termination. `chrome.storage.local` stores only the global enable setting, disabled site origins, theme preference, and acknowledgement of this notice.

# Permission explanation

`sidePanel` displays the Treease graph beside the page. `storage` stores the user's enable and site-pause preferences. `<all_urls>` lets the extension inspect the user-clicked element and a small nearby DOM range to decide whether it is JSON. It is not used for browsing-history collection or background page scanning.
