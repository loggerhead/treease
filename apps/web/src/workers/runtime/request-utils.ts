
// 职责：operation 输入归一化；response 与错误由 worker-transport 统一处理。
type MessageWithNestOptions = {
  options?: Record<string, unknown> | undefined;
  nest?: boolean;
};

type MessageWithTextInput = {
  text?: string;
  textBytes?: ArrayBuffer | SharedArrayBuffer | null;
};

export function withNestOptions<T extends MessageWithNestOptions>(
  message: T,
): Record<string, unknown> & { nest: boolean } {
  return {
    ...message.options,
    nest: Boolean(message.nest ?? message.options?.nest),
  };
}

export function readWorkerTextInput(message: MessageWithTextInput): {
  text: string;
  textBytes: ArrayBuffer | SharedArrayBuffer | null;
  hasBytes: boolean;
  resolvedText: string;
} {
  const textBytes =
    message && typeof message === 'object' && 'textBytes' in message
      ? (message.textBytes as ArrayBuffer | SharedArrayBuffer | null)
      : null;
  const hasBytes =
    !!textBytes &&
    (textBytes instanceof ArrayBuffer ||
      (typeof SharedArrayBuffer !== 'undefined' && textBytes instanceof SharedArrayBuffer));
  const text = typeof message.text === 'string' ? message.text : '';
  return {
    text,
    textBytes,
    hasBytes,
    resolvedText: text || (hasBytes ? decodeWorkerTextBytes(textBytes) : ''),
  };
}

export function decodeWorkerTextBytes(textBytes: ArrayBuffer | SharedArrayBuffer): string {
  if (typeof TextDecoder !== 'function') return '';
  return new TextDecoder().decode(new Uint8Array(textBytes));
}
