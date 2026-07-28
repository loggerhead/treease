---
summary: "Chrome Side Panel opening contract and Treease fallback behavior."
read_when:
  - Changing extension click-to-panel routing.
---

# Side Panel opening contract

`chrome.sidePanel.open({ tabId })` is called directly from the Service Worker message handler that receives a user-originated Content Script click. Chrome documents this as a valid extension user gesture path.

The call can still reject because Chrome does not guarantee gesture propagation across every browser version or page lifecycle. Treease therefore keeps the latest valid structured document only in tab-scoped Service Worker memory for five minutes. On rejection it sends a temporary page hint that directs the user to the extension action; `openPanelOnActionClick` then opens the native panel and reads the tab state.

The data is intentionally not placed in `chrome.storage`, IndexedDB, a URL, or any network request. Service Worker termination may release it early; this is the intended privacy boundary.

Manual long-run validation: load `apps/extension/dist`, acknowledge the notice, click a JSON `pre`, leave the panel closed, then click the toolbar action within five minutes. Verify the graph appears. Repeat after navigation and after five minutes: the panel must show its empty state. The implementation intentionally does not promise recovery after a Service Worker restart.
