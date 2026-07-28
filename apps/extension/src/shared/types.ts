export type JsonRootType = 'object' | 'array' | 'scalar';
export type StructuredLanguage = 'json' | 'yaml' | 'toml';
export type CandidateKind = 'whole' | 'embedded';
export type PanelStatus = 'empty' | 'loading' | 'ready' | 'invalid' | 'too_large' | 'graph_error';

export type ExtensionSettings = {
  enabled: boolean;
  disabledOrigins: string[];
  allowlist: string[];
  blocklist: string[];
  theme: 'system' | 'light' | 'dark';
  privacyAcknowledged: boolean;
};

export type CandidatePayload = {
  text: string;
  sourceTag: string;
  /** CSS-like ancestry of the clicked DOM element, excluding page text. */
  domPath: string;
  sourceLength: number;
  pageTitle: string;
  pageOrigin: string;
  frameId?: number;
};

export type ReadyDocument = CandidatePayload & {
  rootType: JsonRootType;
  language: StructuredLanguage;
  candidateKind: CandidateKind;
  expiresAt: number;
};

export type PanelState =
  | { status: 'empty' }
  | { status: 'loading'; pageTitle: string; pageOrigin: string }
  | { status: 'ready'; document: ReadyDocument }
  | { status: 'invalid'; message: string; position: number | null; pageTitle: string; pageOrigin: string }
  | { status: 'too_large'; sourceLength: number; pageTitle: string; pageOrigin: string }
  | { status: 'graph_error'; message: string; document: ReadyDocument };

/** Canonical graph projection facts emitted by Treease Core and normalized by Web. */
import type { GraphEdge, GraphNode } from '@treease-web/lib/graph/graph-viewer-render';

// Keep the full normalized Web GraphViewer shape. Reducing this to a display-only
// DTO was what caused the extension to fall back to its own, incompatible SVG.
export type { GraphEdge, GraphNode };

export type GraphData = { nodes: GraphNode[]; edges: GraphEdge[]; coreGraphAvailable: true };
