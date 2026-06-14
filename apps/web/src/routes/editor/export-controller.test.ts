// 职责：export-controller 的单元测试
import { describe, expect, it } from 'vitest';
import { resolveExportDownloadDetails, resolveExportPreviewDetails, type ExportFormatOption } from './export-controller';

const formatOptions: ExportFormatOption[] = [
  { id: 'json', label: 'JSON', extensions: ['.json'] },
  { id: 'yaml', label: 'YAML', extensions: ['.yaml'] },
  { id: 'toml', label: 'TOML', extensions: ['.toml'] },
];

describe('export-controller', () => {
  it('keeps same-format preview feedback generic', () => {
    expect(
      resolveExportPreviewDetails({
        sourceLanguage: 'json',
        targetFormat: 'json',
        formatOptions,
      }),
    ).toEqual({
      previewLanguage: 'json',
      toastMessage: 'Previewed source content',
    });
  });

  it('describes cross-format preview with labels', () => {
    expect(
      resolveExportPreviewDetails({
        sourceLanguage: 'json',
        targetFormat: 'yaml',
        formatOptions,
      }),
    ).toEqual({
      previewLanguage: 'yaml',
      toastMessage: 'Previewed JSON to YAML',
    });
  });

  it('falls back to the editor default language for unknown preview formats', () => {
    expect(
      resolveExportPreviewDetails({
        sourceLanguage: 'json',
        targetFormat: 'custom-format',
        formatOptions,
      }),
    ).toEqual({
      previewLanguage: 'json',
      toastMessage: 'Previewed JSON to custom-format',
    });
  });

  it('builds download filename and both toasts for converted exports', () => {
    expect(
      resolveExportDownloadDetails({
        sourceLanguage: 'json',
        targetFormat: 'yaml',
        tabName: 'example',
        formatOptions,
      }),
    ).toEqual({
      fileName: 'example.yaml',
      toastMessages: ['Converted JSON to YAML', 'Downloaded YAML file'],
    });
  });

  it('uses default tab name and fallback extension when metadata is missing', () => {
    expect(
      resolveExportDownloadDetails({
        sourceLanguage: 'json',
        targetFormat: 'custom-format',
        tabName: '',
        formatOptions,
      }),
    ).toEqual({
      fileName: 'treease.custom-format',
      toastMessages: ['Converted JSON to custom-format', 'Downloaded custom-format file'],
    });
  });
});
