import { describe, expect, it } from 'vitest';
import { IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD, IMPORT_FILE_CHUNK_BYTE_SIZE } from './import-config';

describe('import-config', () => {
  it('keeps file read chunks positive', () => {
    expect(IMPORT_FILE_CHUNK_BYTE_SIZE).toBeGreaterThan(0);
  });

  it('keeps editor flush threshold at least one file chunk', () => {
    expect(IMPORT_EDITOR_FLUSH_BYTE_THRESHOLD).toBeGreaterThanOrEqual(IMPORT_FILE_CHUNK_BYTE_SIZE);
  });
});
