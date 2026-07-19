// Responsibility: create and clear Worker document-runtime state.
import type { SearchIndexEntry } from './graph-search-types';

export type DocumentAnalysisCacheRuntime = {
  encoder: TextEncoder;
  searchIndexByDocumentKey: Map<string, SearchIndexEntry>;
};

export function createDocumentRuntimeState(encoder: TextEncoder): DocumentAnalysisCacheRuntime {
  return {
    encoder,
    searchIndexByDocumentKey: new Map<string, SearchIndexEntry>(),
  };
}

export function clearDocumentRuntimeState(state: DocumentAnalysisCacheRuntime): void {
  state.searchIndexByDocumentKey.clear();
}
