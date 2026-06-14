import { convertJson, formatJson, minifyJson, runYqText, sortText } from '@core-wasm/index';
import type { WorkerContext, WorkerRequest } from './protocol';
import { postHandlerResult, readWorkerTextInput, withNestOptions } from './request-utils';

export async function handleFormat(
  _ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'format' }>,
): Promise<void> {
  await postHandlerResult(_ctx, message, async () => {
    const { resolvedText } = readWorkerTextInput(message);
    const result = await formatJson({
      language: message.language,
      text: resolvedText,
      indent: message.options?.indent,
      sortKeys: message.options?.sortKeys,
    });
    return result.text;
  });
}

export async function handleMinify(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'minify' }>,
): Promise<void> {
  await postHandlerResult(ctx, message, async () => {
    const result = await minifyJson({ language: message.language, text: message.text });
    return result.text;
  });
}

export async function handleSort(ctx: WorkerContext, message: Extract<WorkerRequest, { type: 'sort' }>): Promise<void> {
  await postHandlerResult(ctx, message, () => sortText(message.language, message.text, withNestOptions(message)));
}

export async function handleConvert(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'convert' }>,
): Promise<void> {
  await postHandlerResult(ctx, message, async () => {
    const result = await convertJson({
      sourceLanguage: message.sourceLanguage,
      targetFormat: message.targetFormat,
      text: message.text,
      indent: message.options?.indent,
    });
    return result.text;
  });
}

export async function handleRunYq(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'runYq' }>,
): Promise<void> {
  await postHandlerResult(ctx, message, () =>
    runYqText(message.language, message.text, message.expression, withNestOptions(message)),
  );
}
