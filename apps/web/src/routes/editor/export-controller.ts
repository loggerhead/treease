// 职责：export 控制器：导出预览/下载的语言、文件名与提示文案
import {
  editorLanguageFallback,
  supportedEditorLanguageSet,
  type SupportedEditorLanguageId,
} from '../../lib/monaco/language-support';

export type ExportFormatOption = {
  id: string;
  label: string;
  extensions: string[];
};

type ExportDetailsInput = {
  sourceLanguage: string;
  targetFormat: string;
  formatOptions: ExportFormatOption[];
};

export type ExportPreviewDetails = {
  previewLanguage: SupportedEditorLanguageId;
  toastMessage: string;
};

export type ExportDownloadDetails = {
  fileName: string;
  toastMessages: string[];
};


function resolvePreviewLanguage(targetFormat: string): SupportedEditorLanguageId {
  return supportedEditorLanguageSet.has(targetFormat as SupportedEditorLanguageId)
    ? (targetFormat as SupportedEditorLanguageId)
    : editorLanguageFallback;
}

function resolveFormatLabels(input: ExportDetailsInput): { sourceLabel: string; targetLabel: string } {
  const sourceLabel = input.formatOptions.find((item) => item.id === input.sourceLanguage)?.label ?? input.sourceLanguage;
  const targetLabel = input.formatOptions.find((item) => item.id === input.targetFormat)?.label ?? input.targetFormat;
  return { sourceLabel, targetLabel };
}

export function resolveExportPreviewDetails(input: ExportDetailsInput): ExportPreviewDetails {
  const { sourceLabel, targetLabel } = resolveFormatLabels(input);
  return {
    previewLanguage: resolvePreviewLanguage(input.targetFormat),
    toastMessage:
      input.sourceLanguage === input.targetFormat ? 'Previewed source content' : `Previewed ${sourceLabel} to ${targetLabel}`,
  };
}

export function resolveExportDownloadDetails(
  input: ExportDetailsInput & { tabName: string | null | undefined },
): ExportDownloadDetails {
  const { sourceLabel, targetLabel } = resolveFormatLabels(input);
  const extension = input.formatOptions.find((item) => item.id === input.targetFormat)?.extensions[0] ?? `.${input.targetFormat}`;
  const toastMessages =
    input.sourceLanguage === input.targetFormat
      ? [`Downloaded ${targetLabel} file`]
      : [`Converted ${sourceLabel} to ${targetLabel}`, `Downloaded ${targetLabel} file`];

  return {
    fileName: `${input.tabName || 'treease'}${extension}`,
    toastMessages,
  };
}
