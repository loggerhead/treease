// 职责：Worker 文档运行时状态工厂：createDocumentRuntimeState、clearDocumentRuntimeState
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
