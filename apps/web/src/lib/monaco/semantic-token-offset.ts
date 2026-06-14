const SEMANTIC_TOKEN_FIELDS = 5;

function emptySemanticTokens(): ArrayBuffer {
  return new ArrayBuffer(0);
}

function toUint32Array(tokens: ArrayBuffer | Uint32Array): Uint32Array | null {
  if (tokens instanceof Uint32Array) return tokens;
  if (tokens.byteLength % Uint32Array.BYTES_PER_ELEMENT !== 0) return null;
  return new Uint32Array(tokens);
}

export function offsetSemanticTokens(
  tokens: ArrayBuffer | Uint32Array,
  startLineNumber: number,
  startColumn: number,
): ArrayBuffer {
  const source = toUint32Array(tokens);
  if (!source || source.length === 0) return emptySemanticTokens();
  if (source.length % SEMANTIC_TOKEN_FIELDS !== 0) return emptySemanticTokens();

  const lineOffset = Math.max(0, startLineNumber - 1);
  const firstLineColumnOffset = Math.max(0, startColumn - 1);
  const shifted = new Uint32Array(source.length);

  let blockLine = 0;
  let blockChar = 0;
  let previousSourceLine = 0;
  let previousSourceChar = 0;

  for (let index = 0; index < source.length; index += SEMANTIC_TOKEN_FIELDS) {
    const deltaLine = source[index];
    const deltaStartChar = source[index + 1];
    if (deltaLine === 0) {
      blockChar += deltaStartChar;
    } else {
      blockLine += deltaLine;
      blockChar = deltaStartChar;
    }

    const sourceLine = blockLine + lineOffset;
    const sourceChar = blockChar + (blockLine === 0 ? firstLineColumnOffset : 0);
    const outputDeltaLine = sourceLine - previousSourceLine;
    const outputDeltaStartChar = outputDeltaLine === 0 ? sourceChar - previousSourceChar : sourceChar;

    shifted[index] = outputDeltaLine;
    shifted[index + 1] = outputDeltaStartChar;
    shifted[index + 2] = source[index + 2];
    shifted[index + 3] = source[index + 3];
    shifted[index + 4] = source[index + 4];

    previousSourceLine = sourceLine;
    previousSourceChar = sourceChar;
  }

  return shifted.buffer as ArrayBuffer;
}
