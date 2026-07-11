import { convertJson, formatJson, minifyJson, runYqText, sortText } from '@core-wasm/index';
import type { WorkerRequest } from './protocol';
import { readWorkerTextInput, withNestOptions } from './request-utils';

export async function handleFormat(
  message: Extract<WorkerRequest, { type: 'format' }>,
): Promise<string> {
  const { resolvedText } = readWorkerTextInput(message);
  const result = await formatJson({
    language: message.language,
    text: resolvedText,
    indent: message.options?.indent,
    nest: withNestOptions(message).nest,
    sortKeys: message.options?.sortKeys,
  });
  return result.text;
}

export async function handleMinify(
  message: Extract<WorkerRequest, { type: 'minify' }>,
): Promise<string> {
  const result = await minifyJson({ language: message.language, text: message.text });
  return result.text;
}

export async function handleSort(message: Extract<WorkerRequest, { type: 'sort' }>): Promise<string> {
  return sortText(message.language, message.text, withNestOptions(message));
}

export async function handleConvert(
  message: Extract<WorkerRequest, { type: 'convert' }>,
): Promise<string> {
  const result = await convertJson({
    sourceLanguage: message.sourceLanguage,
    targetFormat: message.targetFormat,
    text: message.text,
    indent: message.options?.indent,
  });
  return result.text;
}

export async function handleRunYq(
  message: Extract<WorkerRequest, { type: 'runYq' }>,
): Promise<string> {
  return runYqText(message.language, message.text, message.expression, withNestOptions(message));
}
