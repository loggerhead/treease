import { convertJson, formatJson, minifyJson, runYqText, sortText } from '@core-wasm/index';
import type { WorkerRequest } from './protocol';
import { readWorkerTextInput, withNestOptions } from './request-utils';

export async function handleFormat(
  message: Extract<WorkerRequest, { type: 'format' }>,
): Promise<string> {
  const { resolvedText } = readWorkerTextInput(message);
  try {
    const result = await formatJson({
      language: message.language,
      text: resolvedText,
      indent: message.options?.indent,
      smart: message.options?.smart,
      maxLineLength: message.options?.maxLineLength,
      maxInlineComplexity: message.options?.maxInlineComplexity,
      maxArrayInlineItems: message.options?.maxArrayInlineItems,
      alignObjectArrays: message.options?.alignObjectArrays,
      nest: withNestOptions(message).nest,
      sortKeys: message.options?.sortKeys,
    });
    return result.text;
  } catch (error) {
    // A compare-sidecar deliberately accepts incomplete source so Compare can
    // render its parse/raw outcome. This opt-in transport outcome avoids
    // presenting expected invalid input as a worker failure.
    if (message.options?.allowInvalidSource === true) return resolvedText;
    throw error;
  }
}

export async function handleMinify(
  message: Extract<WorkerRequest, { type: 'minify' }>,
): Promise<string> {
  const result = await minifyJson({ language: message.language, text: message.text });
  return result.text;
}

export async function handleCompact(
  message: Extract<WorkerRequest, { type: 'compact' }>,
): Promise<string> {
  return runYqText(message.language, message.text, 'compact', withNestOptions(message));
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
