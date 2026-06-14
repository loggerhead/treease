// 职责：文档分析结果解析器：从 Worker 获取 analysis 并转换为 UI 可消费结构
import type { DocumentAnalysisResult } from '../../shared/worker-protocol/protocol';


export type ResolvableDocumentAnalysis = Partial<
  Pick<
    DocumentAnalysisResult,
    'documentKey' | 'tree' | 'value' | 'diagnostics' | 'semanticTokens' | 'semanticTokenVersion' | 'sourceByteLength' | 'language'
  >
>;

export type ResolvedDocumentAnalysis =
  | { status: 'unknown'; analysis: null }
  | { status: 'resolved'; analysis: ResolvableDocumentAnalysis };

export async function resolveDocumentAnalysis(options: {
  documentKey: string;
  preloadedAnalysis?: ResolvableDocumentAnalysis | null;
}): Promise<ResolvedDocumentAnalysis> {
  if (!options.documentKey || options.preloadedAnalysis == null) {
    return { status: 'unknown', analysis: null };
  }
  return { status: 'resolved', analysis: options.preloadedAnalysis };
}
