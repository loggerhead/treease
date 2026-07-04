import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../wasm/wasm-worker-singleton', () => ({
  callSharedWasmWorker: vi.fn(),
}));

import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import {
  diagnosticsResultFromErrors,
  readStoredDiagnosticsResult,
} from './EditorDiagnostics';

describe('EditorDiagnostics', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('does not suppress preloaded diagnostics for blank json input', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('   ') } as any;

    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag/blank-json',
      true,
      [{ startLineNumber: 1, startColumn: 4, endLineNumber: 1, endColumn: 4, kind: 1 }],
    );

    expect(result.markers).toHaveLength(1);
    expect(result.markers[0]).toMatchObject({
      startLineNumber: 1,
      startColumn: 4,
      endLineNumber: 1,
      endColumn: 4,
      message: 'Syntax error',
      severity: 8,
    });
    expect(result.diagnostics).toHaveLength(1);
    expect(callSharedWasmWorker).not.toHaveBeenCalled();
  });

  it('sorts diagnostics by position and preserves all errors', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const text = ['line1', 'line2 bad', 'line3'].join('\n');
    const model = { getValue: vi.fn().mockReturnValue(text) } as any;

    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag',
      true,
      [
        { startLineNumber: 3, startColumn: 1, endLineNumber: 3, endColumn: 3, kind: 1 },
        { startLineNumber: 2, startColumn: 2, endLineNumber: 2, endColumn: 6, kind: 1 },
      ],
    );

    expect(result.markers).toHaveLength(2);
    expect(result.markers[0]).toMatchObject({
      startLineNumber: 2,
      startColumn: 2,
      endLineNumber: 2,
      endColumn: 6,
      message: 'Syntax error',
      severity: 8,
    });
    expect(result.markers[1]).toMatchObject({
      startLineNumber: 3,
      startColumn: 1,
      endLineNumber: 3,
      endColumn: 3,
      message: 'Syntax error',
      severity: 8,
    });

    expect(result.diagnostics).toHaveLength(2);
    expect(result.diagnostics[0].context.map((item) => item.lineNumber)).toEqual([1, 2, 3]);

    expect(callSharedWasmWorker).not.toHaveBeenCalled();
  });

  it('builds diagnostics directly from provided errors without worker calls', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('line1\nline2 bad\nline3') } as any;

    const result = await diagnosticsResultFromErrors(
      monaco,
      model,
      'json' as any,
      [{ startLineNumber: 2, startColumn: 2, endLineNumber: 2, endColumn: 6, kind: 1 }],
      true,
    );

    expect(result.markers[0]).toMatchObject({
      startLineNumber: 2,
      startColumn: 2,
      endLineNumber: 2,
      endColumn: 6,
      message: 'Syntax error',
      severity: 8,
    });
    expect(result.diagnostics[0]?.context.map((item) => item.lineNumber)).toEqual([1, 2, 3]);
    expect(callSharedWasmWorker).not.toHaveBeenCalled();
  });

  it('deduplicates identical diagnostics before rendering', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('line1\nline2 bad\nline3') } as any;

    const result = await diagnosticsResultFromErrors(
      monaco,
      model,
      'json' as any,
      [
        { startLineNumber: 2, startColumn: 2, endLineNumber: 2, endColumn: 6, kind: 1 },
        { startLineNumber: 2, startColumn: 2, endLineNumber: 2, endColumn: 6, kind: 1 },
      ],
      true,
    );

    expect(result.markers).toHaveLength(1);
    expect(result.diagnostics).toHaveLength(1);
  });

  it('uses Missing node message when kind is 2', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{"a":1,') } as any;

    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag/missing-node',
      true,
      [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 2 }],
    );

    expect(result.markers[0]?.message).toBe('Missing node');
    expect(result.diagnostics[0]?.message).toBe('Missing node');
  });

  it('returns empty result when documentKey is empty', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{"a":1}') } as any;

    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce({ diagnostics: [] });

    const result = await readStoredDiagnosticsResult(monaco, model, 'json' as any, '', true);

    expect(result).toEqual({ markers: [], diagnostics: [], error: '' });
    expect(callMock).not.toHaveBeenCalled();
  });

  it('returns empty markers when preloaded diagnostics are empty', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{"a":1}') } as any;

    const result = await readStoredDiagnosticsResult(monaco, model, 'json' as any, 'vitest://diag/no-errors', true, []);

    expect(result).toEqual({ markers: [], diagnostics: [], error: '' });
  });

  it('uses preloaded diagnostics when provided', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{"a":1,') } as any;
    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag/stored-first',
      true,
      [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 1 }],
    );

    expect(result.markers).toHaveLength(1);
    expect(callSharedWasmWorker).not.toHaveBeenCalled();
  });

  it('returns empty diagnostics when no diagnostics are preloaded', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{"a":1,') } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);

    const result = await readStoredDiagnosticsResult(monaco, model, 'json' as any, 'vitest://diag/stored-fallback', true);

    expect(result).toEqual({ markers: [], diagnostics: [], error: '' });
    expect(callMock).not.toHaveBeenCalled();
  });

  it('maps malformed diagnostic payloads to a generic syntax marker', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{"a":1,') } as any;
    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag/malformed',
      true,
      [
        { startLineNumber: undefined, startColumn: undefined, endLineNumber: undefined, endColumn: undefined, kind: 999 } as any,
      ],
    );

    expect(result.markers).toHaveLength(1);
    expect(result.markers[0]).toMatchObject({
      startLineNumber: 1,
      startColumn: 1,
      endLineNumber: 1,
      endColumn: 1,
      message: 'Syntax error',
    });
  });

  it('maps preloaded diagnostics for multi-document json input', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{"a":1}\n{"b":2}\n') } as any;
    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag/multi-document-json',
      true,
      [{ startLineNumber: 2, startColumn: 1, endLineNumber: 2, endColumn: 2, kind: 1 }],
    );

    expect(result.markers).toHaveLength(1);
    expect(result.markers[0]).toMatchObject({
      startLineNumber: 2,
      startColumn: 1,
      endLineNumber: 2,
      endColumn: 2,
      message: 'Syntax error',
      severity: 8,
    });
    expect(result.diagnostics).toHaveLength(1);
    expect(result.diagnostics[0]).toMatchObject({
      startLineNumber: 2,
      startColumn: 1,
      endLineNumber: 2,
      endColumn: 2,
      message: 'Syntax error',
    });
    expect(callSharedWasmWorker).not.toHaveBeenCalled();
  });

  it('does not suppress diagnostics for a single json line', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{"a":1}\n') } as any;
    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag/single-json-line',
      true,
      [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 1 }],
    );

    expect(result.markers).toHaveLength(1);
    expect(callSharedWasmWorker).not.toHaveBeenCalled();
  });

  it('does not suppress diagnostics for pretty-printed json object', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('{\n  "name": "alice",\n  "count": 42,\n  "active": true\n}\n') } as any;
    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag/pretty-json',
      true,
      [{ startLineNumber: 2, startColumn: 3, endLineNumber: 2, endColumn: 9, kind: 1 }],
    );

    expect(result.markers).toHaveLength(1);
    expect(callSharedWasmWorker).not.toHaveBeenCalled();
  });

  it('does not suppress diagnostics for multiline scalar json values', async () => {
    const monaco = { MarkerSeverity: { Error: 8 } } as any;
    const model = { getValue: vi.fn().mockReturnValue('"alice"\n123\ntrue\n') } as any;
    const result = await readStoredDiagnosticsResult(
      monaco,
      model,
      'json' as any,
      'vitest://diag/scalars',
      true,
      [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 7, kind: 1 }],
    );

    expect(result.markers).toHaveLength(1);
    expect(callSharedWasmWorker).not.toHaveBeenCalled();
  });

});
