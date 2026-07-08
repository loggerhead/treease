import { beforeAll, describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { diffText, initWasm } from '@core-wasm/index';

const DiffType = {
  Insert: 0,
  Delete: 1,
} as const;

type FlatDiff = {
  byteOffset: number;
  byteLength: number;
  type: number | undefined;
};

type RawDiff = {
  byteOffset: number;
  byteLength: number;
  type?: number;
  inlineDiffs?: RawDiff[];
};

type RawDiffPair = {
  hasLeft: boolean | number;
  hasRight: boolean | number;
  left?: RawDiff;
  right?: RawDiff;
};

type RawDiffResult = {
  pairs?: RawDiffPair[];
};

function cloneWasmBytes(path: string): ArrayBuffer {
  const bytes = readFileSync(path);
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
}

function newDiff(byteOffset: number, byteLength: number, type: number): FlatDiff {
  return { byteOffset, byteLength, type };
}

function sortDiffs(diffs: FlatDiff[]): FlatDiff[] {
  return [...diffs].sort((left, right) => {
    const leftPriority = left.type === DiffType.Delete ? 0 : left.type === DiffType.Insert ? 1 : Number.MAX_SAFE_INTEGER;
    const rightPriority =
      right.type === DiffType.Delete ? 0 : right.type === DiffType.Insert ? 1 : Number.MAX_SAFE_INTEGER;
    if (leftPriority !== rightPriority) {
      return leftPriority - rightPriority;
    }
    if (left.byteOffset !== right.byteOffset) return left.byteOffset - right.byteOffset;
    return left.byteLength - right.byteLength;
  });
}

function collectHunks(result: RawDiffResult): FlatDiff[] {
  const hunks: FlatDiff[] = [];
  for (const pair of result.pairs ?? []) {
    if (pair.hasLeft && pair.left) {
      hunks.push({ byteOffset: pair.left.byteOffset, byteLength: pair.left.byteLength, type: pair.left.type });
    }
    if (pair.hasRight && pair.right) {
      hunks.push({ byteOffset: pair.right.byteOffset, byteLength: pair.right.byteLength, type: pair.right.type });
    }
  }
  return sortDiffs(hunks);
}

function collectInlines(result: RawDiffResult): FlatDiff[] {
  const inlines: FlatDiff[] = [];
  for (const pair of result.pairs ?? []) {
    for (const side of [pair.left, pair.right]) {
      for (const inline of side?.inlineDiffs ?? []) {
        inlines.push({ byteOffset: inline.byteOffset, byteLength: inline.byteLength, type: inline.type });
      }
    }
  }
  return sortDiffs(inlines);
}

function expectDiffVectors(
  result: RawDiffResult,
  expected: { hunks?: FlatDiff[]; inlines?: FlatDiff[] },
): void {
  if (expected.hunks) {
    expect(collectHunks(result)).toEqual(expected.hunks);
  }
  if (expected.inlines) {
    expect(collectInlines(result)).toEqual(expected.inlines);
  }
}

beforeAll(async () => {
  const wasmPath = fileURLToPath(new URL('../../../../../packages/core/wasm/pkg/core.wasm', import.meta.url));
  await initWasm({ wasmBytes: cloneWasmBytes(wasmPath) });
}, 5_000);

describe('ported compare upstream cases via real wasm diffText', () => {
  it('empty text compare stays diff-free', async () => {
    const result = (await diffText('', '')) as RawDiffResult;

    expectDiffVectors(result, {
      hunks: [],
      inlines: [],
    });
  });

  it('char compare preserves exact inline diff vectors', async () => {
    const result = (await diffText('  "foo": "abc" }', '{ "foo": "adc" }')) as RawDiffResult;

    expect(collectInlines(result)).toEqual([
      newDiff(0, 1, DiffType.Delete),
      newDiff(11, 1, DiffType.Delete),
      newDiff(0, 1, DiffType.Insert),
      newDiff(11, 1, DiffType.Insert),
    ]);
  });

  it('char compare multiline punctuation case preserves exact inline vectors', async () => {
    const left = `[
     ,
    2
 `;
    const right = `[
    1,
    2
]`;

    const result = (await diffText(left, right)) as RawDiffResult;

    expect(collectInlines(result)).toEqual([
      newDiff(6, 1, DiffType.Delete),
      newDiff(15, 1, DiffType.Delete),
      newDiff(6, 1, DiffType.Insert),
      newDiff(15, 1, DiffType.Insert),
    ]);
  });

  it('simple delete hunk keeps exact span', async () => {
    const result = (await diffText('12345', '')) as RawDiffResult;

    expectDiffVectors(result, {
      hunks: [newDiff(0, 5, DiffType.Delete)],
    });
  });

  it('simple insert hunk keeps exact span', async () => {
    const result = (await diffText('', '12345')) as RawDiffResult;

    expectDiffVectors(result, {
      hunks: [newDiff(0, 5, DiffType.Insert)],
    });
  });

  it('a -> a2345 keeps both hunk and inline diff types', async () => {
    const result = (await diffText('a', 'a2345')) as RawDiffResult;

    expect(collectHunks(result)).toEqual([
      newDiff(0, 1, DiffType.Delete),
      newDiff(0, 5, DiffType.Insert),
    ]);
    expect(collectInlines(result)).toEqual([newDiff(1, 4, DiffType.Insert)]);
  });

  it('inline URL character insertion keeps exact offset', async () => {
    const left = '{"link": "<a href=\\"http://google.com/\\">Google</a>"}';
    const right = '{"link": "<a href=\\"http://googlex.com/\\">Google</a>"}';
    const result = (await diffText(left, right)) as RawDiffResult;

    expectDiffVectors(result, {
      inlines: [newDiff(33, 1, DiffType.Insert)],
    });
  });

  it('viewzone error case keeps single-line newline deletion hunk', async () => {
    const result = (await diffText('{\n\n  return tokens;\n}', '{\n  return tokens;\n}')) as RawDiffResult;

    expect(collectHunks(result)).toEqual([newDiff(2, 1, DiffType.Delete)]);
  });

  it('human-readable case preserves exact hunks and inline vectors', async () => {
    const left = String.raw`{
    "Aidan Gillen": {
        "array": [
            "Game of Thron\\"es",
            "The Wire"
        ],
        "string": "some string",
        "int": 2,
        "aboolean": true,
        "boolean": true,
        "null": null,
        "a_null": null,
        "another_null": "null check",
        "object": {
            "foo": "bar",
            "object1": {
                "new prop1": "new prop value"
            },
            "object2": {
                "new prop1": "new prop value"
            },
            "object3": {
                "new prop1": "new prop value"
            },
            "object4": {
                "new prop1": "new prop value"
            }
        }
    },
    "Amy Ryan": {
        "one": "In Treatment",
        "two": "The Wire"
    },
    "Annie Fitzgerald": [
        "Big Love",
        "True Blood"
    ],
    "Anwan Glover": [
        "Treme",
        "The Wire"
    ],
    "Alexander Skarsgard": [
        "Generation Kill",
        "True Blood"
    ],
    "Clarke Peters": null
}`;
    const right = `{
    "Aidan Gillen": {
        "array": [
            "Game of Thrones",
            "The Wire"
        ],
        "string": "some string",
        "int": "2",
        "otherint": 4,
        "aboolean": "true",
        "boolean": false,
        "null": null,
        "a_null": 88,
        "another_null": null,
        "object": {
            "foo": "bar"
        }
    },
    "Amy Ryan": [
        "In Treatment",
        "The Wire"
    ],
    "Annie Fitzgerald": [
        "True Blood",
        "Big Love",
        "The Sopranos",
        "Oz"
    ],
    "Anwan Glover": [
        "Treme",
        "The Wire"
    ],
    "Alexander Skarsg?rd": [
        "Generation Kill",
        "True Blood"
    ],
    "Alice Farmer": [
        "The Corner",
        "Oz",
        "The Wire"
    ]
}`;

    const result = (await diffText(left, right)) as RawDiffResult;

    expectDiffVectors(result, {
      hunks: [
        newDiff(43, 33, DiffType.Delete),
        newDiff(144, 68, DiffType.Delete),
        newDiff(235, 61, DiffType.Delete),
        newDiff(317, 368, DiffType.Delete),
        newDiff(703, 81, DiffType.Delete),
        newDiff(831, 20, DiffType.Delete),
        newDiff(924, 28, DiffType.Delete),
        newDiff(1008, 25, DiffType.Delete),
        newDiff(43, 30, DiffType.Insert),
        newDiff(141, 96, DiffType.Insert),
        newDiff(260, 51, DiffType.Insert),
        newDiff(332, 24, DiffType.Insert),
        newDiff(374, 67, DiffType.Insert),
        newDiff(468, 21, DiffType.Insert),
        newDiff(510, 36, DiffType.Insert),
        newDiff(619, 28, DiffType.Insert),
        newDiff(703, 82, DiffType.Insert),
      ],
      inlines: [
        newDiff(69, 2, DiffType.Delete),
        newDiff(72, 3, DiffType.Delete),
        newDiff(207, 3, DiffType.Delete),
        newDiff(253, 4, DiffType.Delete),
        newDiff(283, 1, DiffType.Delete),
        newDiff(288, 7, DiffType.Delete),
        newDiff(341, 344, DiffType.Delete),
        newDiff(719, 1, DiffType.Delete),
        newDiff(730, 7, DiffType.Delete),
        newDiff(761, 7, DiffType.Delete),
        newDiff(782, 1, DiffType.Delete),
        newDiff(841, 2, DiffType.Delete),
        newDiff(845, 5, DiffType.Delete),
        newDiff(945, 1, DiffType.Delete),
        newDiff(1013, 5, DiffType.Delete),
        newDiff(1020, 6, DiffType.Delete),
        newDiff(1029, 4, DiffType.Delete),
        newDiff(69, 2, DiffType.Insert),
        newDiff(156, 1, DiffType.Insert),
        newDiff(158, 1, DiffType.Insert),
        newDiff(170, 23, DiffType.Insert),
        newDiff(204, 1, DiffType.Insert),
        newDiff(209, 1, DiffType.Insert),
        newDiff(231, 4, DiffType.Insert),
        newDiff(278, 2, DiffType.Insert),
        newDiff(390, 1, DiffType.Insert),
        newDiff(439, 1, DiffType.Insert),
        newDiff(520, 1, DiffType.Insert),
        newDiff(523, 8, DiffType.Insert),
        newDiff(532, 1, DiffType.Insert),
        newDiff(534, 13, DiffType.Insert),
        newDiff(640, 1, DiffType.Insert),
        newDiff(708, 4, DiffType.Insert),
        newDiff(714, 6, DiffType.Insert),
        newDiff(723, 1, DiffType.Insert),
        newDiff(725, 61, DiffType.Insert),
      ],
    });
  });

  it('viewzone error case 2 preserves long delete/insert hunks', async () => {
    const left = `wordDiff.tokenize = function(value) {
  // All whitespace symbols except newline group into one token, each newline - in separate token
  let tokens = value.split(/([^\\S\\r\\n]+|[()[\\]{}'"\\r\\n]|\\b)/);

  // Join the boundary splits that we do not consider to be boundaries. This is primarily the extended Latin character set.
  for (let i = 0; i < tokens.length - 1; i++) {
    // If we have an empty string in the next field and we have only word chars before and after, merge
    if (!tokens[i + 1] && tokens[i + 2]
          && extendedWordChars.test(tokens[i])
          && extendedWordChars.test(tokens[i + 2])) {
      tokens[i] += tokens[i + 2];
      tokens.splice(i + 1, 2);
      i--;
    }
  }

  return tokens;
};`;
    const right = `wordDiff.tokenize = function(value) {
  const tokens = [];
  let prevCharType = '';
  for (let i = 0; i < value.length; i++) {
    const char = value[i];
    if (spaceRegExp.test(char)) {
      if(prevCharType === 'space') {
        tokens[tokens.length - 1] += ' ';
      } else {
        tokens.push(' ');
      }
      prevCharType = 'space';
    } else if (cannotBecomeWordRegExp.test(char)) {
      tokens.push(char);
      prevCharType = '';
    } else {
      if(prevCharType === 'word') {
        tokens[tokens.length - 1] += char;
      } else {
        tokens.push(char);
      }
      prevCharType = 'word';
    }
  }
  return tokens;
};`;

    const result = (await diffText(left, right)) as RawDiffResult;

    expectDiffVectors(result, {
      hunks: [
        newDiff(38, 654, DiffType.Delete),
        newDiff(703, 1, DiffType.Delete),
        newDiff(38, 580, DiffType.Insert),
      ],
    });
  });

  it('diff error case preserves exact multiline hunks', async () => {
    const left = `hello


b
    }
  }

  return tokens;
};`;
    const right = `world

a
c
            } ;
        }
    } return tokens;
};`;

    const result = (await diffText(left, right)) as RawDiffResult;

    expectDiffVectors(result, {
      hunks: [
        newDiff(0, 5, DiffType.Delete),
        newDiff(7, 2, DiffType.Delete),
        newDiff(16, 21, DiffType.Delete),
        newDiff(0, 5, DiffType.Insert),
        newDiff(7, 19, DiffType.Insert),
        newDiff(37, 20, DiffType.Insert),
      ],
    });
  });

  it('tab-space only differences stay diff-free', async () => {
    const result = (await diffText('{\n  "a": 1\n}', '{\n\t"a": 1\n}')) as RawDiffResult;

    expect(result.pairs ?? []).toEqual([]);
  });
});
