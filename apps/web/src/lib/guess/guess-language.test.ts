import { beforeEach, describe, expect, it, vi } from 'vitest';
import { guessLanguage } from './guess-language';
import * as wasmWorkerSingleton from '../wasm/wasm-worker-singleton';

vi.mock('../wasm/wasm-worker-singleton', async () => {
  const actual = await vi.importActual('../wasm/wasm-worker-singleton');
  return { ...actual, callSharedWasmWorker: vi.fn() };
});


describe('guessLanguage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('delegates to worker', () => {
    it('calls worker with guessLanguage type and text payload', async () => {
      const input = '{"key": "value"}';
      vi.mocked(wasmWorkerSingleton.callSharedWasmWorker).mockResolvedValue('json');
      await guessLanguage(input);
      expect(wasmWorkerSingleton.callSharedWasmWorker).toHaveBeenCalledWith('guessLanguage', { text: input });
    });

    it('returns the worker result', async () => {
      vi.mocked(wasmWorkerSingleton.callSharedWasmWorker).mockResolvedValue('yaml');
      const result = await guessLanguage('name: val');
      expect(result).toBe('yaml');
    });

    it('returns null when worker returns null', async () => {
      vi.mocked(wasmWorkerSingleton.callSharedWasmWorker).mockResolvedValue(null);
      const result = await guessLanguage('');
      expect(result).toBeNull();
    });
  });

  describe('with diagnosticsProvider', () => {
    it('ignores diagnosticsProvider and keeps feature-only json result', async () => {
      const provider = vi.fn(async () => []);
      vi.mocked(wasmWorkerSingleton.callSharedWasmWorker).mockResolvedValue('json');
      const input = '{"key": "value"}';
      const result = await guessLanguage(input, provider);
      expect(result).toBe('json');
      expect(provider).not.toHaveBeenCalled();
    });

    it('ignores diagnosticsProvider and keeps null result', async () => {
      const provider = vi.fn(async () => []);
      vi.mocked(wasmWorkerSingleton.callSharedWasmWorker).mockResolvedValue(null);
      const result = await guessLanguage('abc', provider);
      expect(result).toBeNull();
      expect(provider).not.toHaveBeenCalled();
    });
  });
});
