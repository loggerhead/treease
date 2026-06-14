// 职责：yq 预览控制器：表达式执行、语言推断、友好错误消息、Worker 调用
import type { Settings } from '../../lib/settings/ui-settings';
import type { SupportedEditorLanguageId } from '../../lib/monaco/language-support';

type FormattingSettings = Settings['formatting'];

const yqPreviewLanguageMatchers: Array<{ pattern: RegExp; language: SupportedEditorLanguageId }> = [
  { pattern: /(?:^|[^\w-])to_yaml(?:\s*\(|$)/, language: 'yaml' },
  { pattern: /(?:^|[^\w-])to_json(?:\s*\(|$)/, language: 'json' },
  { pattern: /(?:^|[^\w-])to_toml(?:\s*\(|$)/, language: 'toml' },
  { pattern: /(?:^|[^\w-])@toml(?:$|[^\w-])/, language: 'toml' },
];

export type RunYqPreviewInput = {
  expression: string;
  text: string;
  language: SupportedEditorLanguageId;
  formatting: FormattingSettings;
  enableNest: boolean;
  callWorker: <T>(method: string, input: unknown) => Promise<T>;
};

export type RunYqPreviewResult =
  | { ok: true; result: string; previewLanguage: SupportedEditorLanguageId }
  | { ok: false; error: string };

export function toFriendlyYqErrorMessage(error: unknown): string {
  const message = (error instanceof Error ? error.message : String(error)).trim();
  if (!message) return 'yq failed. Please try again.';
  if (message === 'Expression is required') return 'Enter a yq expression.';
  if (/parse failed/i.test(message)) return 'The source content cannot be parsed yet. Fix it before running yq.';
  if (/OperationFailed/i.test(message)) {
    return 'This yq expression could not be executed. Check the syntax and make sure it matches the current data shape.';
  }
  if (/ParticipleLexer|UnknownToken|^Parse\(/i.test(message)) {
    return 'This yq expression could not be executed. Check the syntax and make sure it matches the current data shape.';
  }
  if (/format options required|format options missing fields/i.test(message)) {
    return 'yq failed. Refresh the page and try again.';
  }
  return message;
}

export function resolveYqPreviewLanguage(expression: string, sourceLanguage: SupportedEditorLanguageId): SupportedEditorLanguageId {
  for (const { pattern, language } of yqPreviewLanguageMatchers) {
    if (pattern.test(expression)) return language;
  }
  return sourceLanguage;
}


export async function runYqPreview(input: RunYqPreviewInput): Promise<RunYqPreviewResult> {
  const expression = input.expression.trim();
  if (!expression) return { ok: false, error: 'Enter a yq expression.' };
  try {
    const previewLanguage = resolveYqPreviewLanguage(expression, input.language);
    const result = await input.callWorker<string>('runYq', {
      language: input.language,
      text: input.text,
      expression,
      options: {
        ...input.formatting,
        nest: input.enableNest,
      },
    });
    return { ok: true, result, previewLanguage };
  } catch (error) {
    return { ok: false, error: toFriendlyYqErrorMessage(error) };
  }
}
