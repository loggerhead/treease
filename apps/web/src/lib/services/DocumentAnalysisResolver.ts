// Responsibility: resolve document-analysis results from the Worker into UI-consumable data.
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
