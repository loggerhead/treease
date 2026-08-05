import type { SupportedEditorLanguageId } from '../../monaco/language-support';

/**
 * Shared, channel-neutral content processing.  It deliberately has no
 * workspace, snapshot, Monaco, or view dependencies: channels own those
 * concerns through their sinks.
 */
export type ContentChannel = 'main-document' | 'sidecar-input';

export type ContentFormatOptions = {
  indent: number;
  smart: boolean;
  maxLineLength: number;
  maxInlineComplexity: number;
  maxArrayInlineItems: number;
  alignObjectArrays: boolean;
  nest: boolean;
};

export type ContentProcessingRequest = {
  channel: ContentChannel;
  language: SupportedEditorLanguageId;
  text: string;
  format: ContentFormatOptions | null;
};

export type ContentProcessingResult = {
  language: SupportedEditorLanguageId;
  text: string;
  semanticTokens: number[];
  formatting: 'applied' | 'not-requested' | 'invalid-source';
};

type CallWasmWorker = (method: string, input: unknown) => Promise<any>;

/**
 * The engine owns computation and freshness sequencing only. A channel-owned
 * sink owns persistence and visible projection; consequently a sidecar sink
 * cannot accidentally acquire Document Runtime authority.
 */
export type ContentTransactionSink<TTarget> = {
  isDocumentCurrent(target: TTarget): boolean;
  commit(target: TTarget, value: Pick<ContentProcessingResult, 'language' | 'text'>): TTarget | null;
  isVisibleCurrent(target: TTarget): boolean;
  project(target: TTarget, result: ContentProcessingResult): void;
};

export type ContentTransactionStatus = 'committed' | 'stale';

export function createContentTransactionEngine(callWasmWorker: CallWasmWorker) {
  async function process(request: ContentProcessingRequest): Promise<ContentProcessingResult> {
    const formatted = await calculateFormattedText(request);
    const text = formatted.text;
    const semanticTokens = await callWasmWorker('semanticTokens', {
      language: request.language,
      text,
    });
    return { language: request.language, text, semanticTokens: semanticTokens.semanticTokens ?? [], formatting: formatted.status };
  }

  async function run<TTarget>(
    request: ContentProcessingRequest,
    target: TTarget,
    sink: ContentTransactionSink<TTarget>,
  ): Promise<ContentTransactionStatus> {
    // Formatting is the only pre-commit asynchronous calculation. For normal
    // typing this reaches commit synchronously, so every edit advances the
    // channel revision before a later keystroke can start.
    const formatted = await calculateFormattedText(request);
    const text = formatted.text;
    if (!sink.isDocumentCurrent(target)) return 'stale';
    const committedTarget = sink.commit(target, { language: request.language, text });
    if (!committedTarget || !sink.isDocumentCurrent(committedTarget)) return 'stale';
    const tokens = await callWasmWorker('semanticTokens', {
      language: request.language,
      text,
    });
    if (!sink.isDocumentCurrent(committedTarget)) return 'stale';
    const result = { language: request.language, text, semanticTokens: tokens.semanticTokens ?? [], formatting: formatted.status };
    if (sink.isVisibleCurrent(committedTarget)) sink.project(committedTarget, result);
    return 'committed';
  }

  async function calculateFormattedText(request: ContentProcessingRequest): Promise<{ text: string; status: ContentProcessingResult['formatting'] }> {
    if (!request.format) return { text: request.text, status: 'not-requested' };
    try {
      return {
        text: await callWasmWorker('format', {
          language: request.language,
          text: request.text,
          options: { ...request.format, allowInvalidSource: true },
        }),
        status: 'applied',
      };
    } catch {
      // Invalid syntax is a valid sidecar input: preserve it as such so the
      // compare surface can report a parseable/raw difference. This is an
      // explicit formatter outcome, not a second routing path.
      return { text: request.text, status: 'invalid-source' };
    }
  }

  return { process, run };
}
