import { describe, expect, it } from 'vitest';

import { resolveDocumentAnalysis } from './DocumentAnalysisResolver';

describe('DocumentAnalysisResolver', () => {
  it('returns preloaded analysis when provided', async () => {
    const result = await resolveDocumentAnalysis({
      documentKey: 'doc-1',
      preloadedAnalysis: {
        diagnostics: [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 1 }],
      },
    });

    expect(result).toEqual({
      status: 'resolved',
      analysis: {
        diagnostics: [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 1 }],
      },
    });
  });

  it('returns unknown when preloaded analysis is missing', async () => {
    const result = await resolveDocumentAnalysis({
      documentKey: 'doc-2',
    });

    expect(result).toEqual({ status: 'unknown', analysis: null });
  });

  it('returns unknown for empty documentKey', async () => {
    const result = await resolveDocumentAnalysis({
      documentKey: '',
    });

    expect(result).toEqual({ status: 'unknown', analysis: null });
  });
});
