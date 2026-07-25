---
summary: "Top-level module boundaries, dependency directions, and architecture diagram."
read_when:
  - You need a high-level view of the relationships among Web, Desktop, Server, WASM, Core, and CLI.
  - You are designing or reviewing a cross-layer boundary.
---

# Treease Architecture Overview

## Single Responsibility

This document answers one question: how Treease's top-level modules are layered and how dependencies flow between them.

## Top-Level Dependency Diagram

```mermaid
flowchart TB
  Desktop["apps/desktop\nTauri Desktop Workspace host"] --> Web

  subgraph Web["apps/web"]
    WebUI["Svelte routes, components, and frontend state"]
    WorkspaceHost["Workspace Host\nbrowser / desktop platform capability seam"]
    Worker["Worker boundary\ntransport, request correlation, UI fan-out"]
    WebUI --> WorkspaceHost
    WebUI --> Worker
  end

  subgraph Wasm["packages/core/wasm"]
    WasmAdapter["TypeScript WASM adapter\ndocument API and compat API"]
    GeneratedProtocol["Generated document protocol binding"]
  end

  subgraph Core["packages/core/src"]
    DocumentWasm["Document Runtime WASM exports\nwasm_document.rs"]
    CompatWasm["Non-document / compatibility WASM exports\nwasm.rs and wasm/"]
    DocumentProtocol["Document protocol source\ndocument/protocol.rs"]
    CoreCapabilities["Document Runtime; parsing, formatting,\nevaluation, operators, and graph construction"]
    DocumentWasm --> CoreCapabilities
    CompatWasm --> CoreCapabilities
    DocumentProtocol --> DocumentWasm
  end

  subgraph HostedApi["Hosted API (separate repository)"]
    ServerApi["API boundary\nauth, billing, sharing, AI, and usage"]
    ExternalServices["Supabase and AI providers"]
    ServerApi --> ExternalServices
  end

  ApiContracts["packages/api-contracts\npublic HTTP request/response schemas"]
  ShareProtocol["packages/share-protocol\npublic share resource schema"]
  CLI["apps/cli\nstandalone command-line application"]

  Worker --> WasmAdapter
  GeneratedProtocol --> Worker
  WasmAdapter --> DocumentWasm
  WasmAdapter --> CompatWasm
  DocumentProtocol -. generates .-> GeneratedProtocol
  WebUI --> ApiContracts
  ServerApi --> ApiContracts
  WebUI -. HTTPS API .-> ServerApi
  WebUI --> ShareProtocol
  ServerApi --> ShareProtocol
  CLI --> CoreCapabilities
```

- `apps/web` is the frontend shared by the browser workspace and desktop application. It owns presentation, interaction, and frontend state; it does not implement Core document computation.
- `apps/desktop` is the Tauri host for desktop packaging and platform capabilities. Shared UI accesses those capabilities through `Workspace Host`, not through direct Tauri API coupling.
- The Hosted API is the product-service boundary for accounts, billing, sharing, AI, and usage. Its implementation is maintained outside this repository and does not duplicate parsing, formatting, evaluation, or graph construction.
- `packages/core` is the sole implementation of document computation. `packages/core/wasm` adapts only its WASM surface, which Web accesses through the Worker.
- `apps/cli` reuses `treease-core` computation while independently owning command-line arguments, I/O, and user-visible CLI contracts.

## Reading the Diagram

- Solid arrows represent dependency or host relationships; dashed arrows represent generated artifacts or runtime calls. Web never calls internal implementations in `packages/core/src` or the Hosted API repository directly.
- `packages/api-contracts` is the public HTTP boundary. It contains client-visible schemas only; Server repositories, provider integrations, and billing models remain private implementation details.
- `packages/share-protocol` owns serialized share resources, while API envelopes and errors belong to `packages/api-contracts`.
- `packages/core/src/document/protocol.rs` is the sole source of truth for the Document Protocol; `packages/core/wasm/document-protocol.generated.ts` is generated output.
- `packages/core/src/wasm_document.rs` is the Document Runtime WASM export boundary. `packages/core/src/wasm.rs` and `packages/core/src/wasm/` contain only non-Document-Runtime or compatibility ABI.
- The Worker is the browser-to-WASM transport, request-correlation, and UI fan-out boundary. See `docs/contracts/document-runtime.md` for Document Runtime authority, freshness, and snapshot semantics.
