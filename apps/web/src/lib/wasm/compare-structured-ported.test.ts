import { beforeAll, describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { initWasm } from '@core-wasm/index';

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
    if (leftPriority !== rightPriority) return leftPriority - rightPriority;
    if (left.byteOffset !== right.byteOffset) return left.byteOffset - right.byteOffset;
    return left.byteLength - right.byteLength;
  });
}

function collectHunks(result: RawDiffResult): FlatDiff[] {
  const hunks: FlatDiff[] = [];
  for (const pair of result.pairs ?? []) {
    if (pair.hasLeft && pair.left) hunks.push({ byteOffset: pair.left.byteOffset, byteLength: pair.left.byteLength, type: pair.left.type });
    if (pair.hasRight && pair.right) hunks.push({ byteOffset: pair.right.byteOffset, byteLength: pair.right.byteLength, type: pair.right.type });
  }
  return sortDiffs(hunks);
}

function collectInlines(result: RawDiffResult): FlatDiff[] {
  const inlines: FlatDiff[] = [];
  for (const pair of result.pairs ?? []) {
    // These upstream compatibility vectors cover only paired replacements.
    // Standalone array additions/deletions intentionally carry their whole
    // node as an inline range and are asserted by presentation tests.
    if (!pair.hasLeft || !pair.hasRight) continue;
    for (const side of [pair.left, pair.right]) {
      for (const inline of side?.inlineDiffs ?? []) {
        inlines.push({ byteOffset: inline.byteOffset, byteLength: inline.byteLength, type: inline.type });
      }
    }
  }
  return sortDiffs(inlines);
}

function expectStructured(result: RawDiffResult, expected: { hunks?: FlatDiff[]; inlines?: FlatDiff[] }): void {
  if (expected.hunks) {
    expect(collectHunks(result)).toEqual(expected.hunks);
  }
  if (expected.inlines) {
    expect(collectInlines(result)).toEqual(expected.inlines);
  }
}

async function diffStructured(language: string, left: string, right: string): Promise<RawDiffResult> {
  const api = (await import('@core-wasm/index')) as Record<string, unknown>;
  const fn = api.diffStructured as ((language: string, left: string, right: string) => Promise<RawDiffResult>) | undefined;
  if (typeof fn !== 'function') {
    throw new Error('diffStructured is unavailable');
  }
  return fn(language, left, right);
}

beforeAll(async () => {
  const wasmPath = fileURLToPath(new URL('../../../../../packages/core/wasm/pkg/core.wasm', import.meta.url));
  await initWasm({ wasmBytes: cloneWasmBytes(wasmPath) });
}, 5_000);

describe('ported structured compare upstream cases via real wasm', () => {
  it('diffVal keeps exact inline vectors for scalar replacement', async () => {
    const result = await diffStructured('json', '{ "foo": "abc" }', '{ "foo": "adc" }');

    expectStructured(result, {
      inlines: [newDiff(11, 1, DiffType.Delete), newDiff(11, 1, DiffType.Insert)],
    });
  });

  it('diffArray keeps exact inline vectors for nested scalar replacement', async () => {
    const result = await diffStructured('json', '[ "foo", "abc" ]', '[ "foo", "adc" ]');

    expectStructured(result, {
      inlines: [newDiff(11, 1, DiffType.Delete), newDiff(11, 1, DiffType.Insert)],
    });
  });

  it('diffArray keeps inserted element hunk at the exact byte span', async () => {
    const result = await diffStructured('json', '[12, 34]', '[12, 23, 34]');

    expectStructured(result, {
      hunks: [newDiff(5, 2, DiffType.Insert)],
      inlines: [],
    });
  });

  it('inconsistent type keeps exact replacement hunks', async () => {
    const cases = [
      {
        left: '{ "akey": [] }',
        right: '{ "akey": null }',
        hunks: [newDiff(10, 2, DiffType.Delete), newDiff(10, 4, DiffType.Insert)],
      },
      {
        left: '{ "akey": null }',
        right: '{ "akey": [] }',
        hunks: [newDiff(10, 4, DiffType.Delete), newDiff(10, 2, DiffType.Insert)],
      },
      {
        left: '{ "akey": {} }',
        right: '{ "akey": null }',
        hunks: [newDiff(10, 2, DiffType.Delete), newDiff(10, 4, DiffType.Insert)],
      },
      {
        left: '{ "akey": null }',
        right: '{ "akey": {} }',
        hunks: [newDiff(10, 4, DiffType.Delete), newDiff(10, 2, DiffType.Insert)],
      },
    ];

    for (const testCase of cases) {
      const result = await diffStructured('json', testCase.left, testCase.right);
      expectStructured(result, { hunks: testCase.hunks, inlines: [] });
    }
  });

  it('object entry replacement keeps exact bound spans', async () => {
    const left = '{"editor.detectIndentation": false,"editor.tabSize": 2,"files.exclude": {".vscode/": true,"foo": "bar"}}';
    const right = '{"editor.detectIndentation": false,"editor.tabSize": 2,"files.exclude": {".slash/": true,"foo": "bar"}}';

    const result = await diffStructured('json', left, right);

    expectStructured(result, {
      hunks: [newDiff(73, 16, DiffType.Delete), newDiff(73, 15, DiffType.Insert)],
    });
  });

  it('object scalar replacement keeps exact inline spans', async () => {
    const left = '{"editor.detectIndentation": false,"editor.tabSize": 2,"files.exclude": {"bas":".vscode/","foo": "bar"}}';
    const right = '{"editor.detectIndentation": false,"editor.tabSize": 2,"files.exclude": {"bas":".slash/","foo": "bar"}}';

    const result = await diffStructured('json', left, right);

    expectStructured(result, {
      inlines: [newDiff(81, 6, DiffType.Delete), newDiff(81, 5, DiffType.Insert)],
    });
  });

  it('guide example keeps exact mixed hunk and inline vectors', async () => {
    const left = `{
    "int64": 12345678987654321,
    "key": "value",
    "array": [
        12345678987654321,
        0.1234567891111111111,
        1,
        2,
        3
    ]
}`;
    const right = `{
    "int64": 12345678987654320,
    "kee": "value",
    "array": [
        12345678987654320,
        0.1234567891111111110,
        2,
        3,
        1
    ]
}`;

    const result = await diffStructured('json', left, right);

    expectStructured(result, {
      hunks: [
        newDiff(15, 17, DiffType.Delete),
        newDiff(38, 14, DiffType.Delete),
        newDiff(77, 17, DiffType.Delete),
        newDiff(104, 21, DiffType.Delete),
        newDiff(135, 1, DiffType.Delete),
        newDiff(15, 17, DiffType.Insert),
        newDiff(38, 14, DiffType.Insert),
        newDiff(77, 17, DiffType.Insert),
        newDiff(104, 21, DiffType.Insert),
        newDiff(157, 1, DiffType.Insert),
      ],
      inlines: [
        newDiff(31, 1, DiffType.Delete),
        newDiff(93, 1, DiffType.Delete),
        newDiff(124, 1, DiffType.Delete),
        newDiff(31, 1, DiffType.Insert),
        newDiff(93, 1, DiffType.Insert),
        newDiff(124, 1, DiffType.Insert),
      ],
    });
  });

  it('numeric array compare keeps exact reorder and inline vectors', async () => {
    const left = `[
    12345678987654321,
    0.1234567891111111111,
    1,
    2,
    3
]`;
    const right = `[
    12345678987654320,
    0.1234567891111111110,
    2,
    3,
    1
]`;

    const result = await diffStructured('json', left, right);

    expectStructured(result, {
      hunks: [
        newDiff(6, 17, DiffType.Delete),
        newDiff(29, 21, DiffType.Delete),
        newDiff(56, 1, DiffType.Delete),
        newDiff(6, 17, DiffType.Insert),
        newDiff(29, 21, DiffType.Insert),
        newDiff(70, 1, DiffType.Insert),
      ],
      inlines: [
        newDiff(22, 1, DiffType.Delete),
        newDiff(49, 1, DiffType.Delete),
        newDiff(22, 1, DiffType.Insert),
        newDiff(49, 1, DiffType.Insert),
      ],
    });
  });

  it('root type mismatch keeps full-document replacement hunks', async () => {
    const cases = [
      {
        left: `{
    "foo": [
        {
            "OBJ_ID": "CN=Kate Smith,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
            "userAccountControl": "512",
            "objectGUID": "b3067a77-875b-4208-9ee3-39128adeb654",
            "lastLogon": "0",
            "sAMAccountName": "ksmith",
            "userPrincipalName": "ksmith@cloudaddc.qalab.cam.novell.com",
            "distinguishedName": "CN=Kate Smith,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
        },
        {
            "OBJ_ID": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
            "userAccountControl": "512",
            "objectGUID": "c3f7dae9-9b4f-4d55-a1ec-bf9ef45061c3",
            "lastLogon": "130766915788304915",
            "sAMAccountName": "tswan",
            "userPrincipalName": "tswan@cloudaddc.qalab.cam.novell.com",
            "distinguishedName": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
        }
    ]
}`,
        right: `[
    {
        "OBJ_ID": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
        "userAccountControl": "512",
        "objectGUID": "c3f7dae9-9b4f-4d55-a1ec-bf9ef45061c3",
        "lastLogon": "130766915788304915",
        "sAMAccountName": "tswan",
        "userPrincipalName": "tswan@cloudaddc.qalab.cam.novell.com",
        "distinguishedName": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
    }
]`,
        hunks: [newDiff(0, 1020, DiffType.Delete), newDiff(0, 475, DiffType.Insert)],
      },
      {
        left: `[
    {
        "OBJ_ID": "CN=Kate Smith,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
        "userAccountControl": "512",
        "objectGUID": "b3067a77-875b-4208-9ee3-39128adeb654",
        "lastLogon": "0",
        "sAMAccountName": "ksmith",
        "userPrincipalName": "ksmith@cloudaddc.qalab.cam.novell.com",
        "distinguishedName": "CN=Kate Smith,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
    },
    {
        "OBJ_ID": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
        "userAccountControl": "512",
        "objectGUID": "c3f7dae9-9b4f-4d55-a1ec-bf9ef45061c3",
        "lastLogon": "130766915788304915",
        "sAMAccountName": "tswan",
        "userPrincipalName": "tswan@cloudaddc.qalab.cam.novell.com",
        "distinguishedName": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
    }
]`,
        right: `{
    "foo": [
        {
            "OBJ_ID": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
            "userAccountControl": "512",
            "objectGUID": "c3f7dae9-9b4f-4d55-a1ec-bf9ef45061c3",
            "lastLogon": "130766915788304915",
            "sAMAccountName": "tswan",
            "userPrincipalName": "tswan@cloudaddc.qalab.cam.novell.com",
            "distinguishedName": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
        }
    ]
}`,
        hunks: [newDiff(0, 929, DiffType.Delete), newDiff(0, 530, DiffType.Insert)],
      },
    ];

    for (const testCase of cases) {
      const result = await diffStructured('json', testCase.left, testCase.right);
      expectStructured(result, { hunks: testCase.hunks });
    }
  });

  it('escape compare keeps exact raw spans and inline additions', async () => {
    const left = `{
    "newline": "a\\nb",
    "slash": "a\\\\b",
    "quotes": "a\\"b",
    "backspace": "a\\bb",
    "formfeed": "a\\fb",
    "carriagereturn": "a\\rb",
    "tab": "a\\tb",
    "a\\nb": "newline",
    "a\\\\b": "slash",
    "a\\"b": "quotes",
    "a\\bb": "backspace",
    "a\\fb": "formfeed",
    "a\\rb": "carriagereturn",
    "a\\tb": "tab"
}`;
    const right = `{
    "newline": "a\\nbx",
    "slash": "a\\\\bx",
    "quotes": "a\\"bx",
    "backspace": "a\\bbx",
    "formfeed": "a\\fbx",
    "carriagereturn": "a\\rbx",
    "tab": "a\\tbx",
    "a\\nb": "newline",
    "a\\\\bx": "slash",
    "a\\"bx": "quotes",
    "a\\bbx": "backspace",
    "a\\fbx": "formfeed",
    "a\\rbx": "carriagereturn",
    "a\\tbx": "tab"
}`;

    const result = await diffStructured('json', left, right);

    expectStructured(result, {
      hunks: [
        newDiff(17, 6, DiffType.Delete),
        newDiff(38, 6, DiffType.Delete),
        newDiff(60, 6, DiffType.Delete),
        newDiff(85, 6, DiffType.Delete),
        newDiff(109, 6, DiffType.Delete),
        newDiff(139, 6, DiffType.Delete),
        newDiff(158, 6, DiffType.Delete),
        newDiff(193, 15, DiffType.Delete),
        newDiff(214, 16, DiffType.Delete),
        newDiff(236, 19, DiffType.Delete),
        newDiff(261, 18, DiffType.Delete),
        newDiff(285, 24, DiffType.Delete),
        newDiff(315, 13, DiffType.Delete),
        newDiff(17, 7, DiffType.Insert),
        newDiff(39, 7, DiffType.Insert),
        newDiff(62, 7, DiffType.Insert),
        newDiff(88, 7, DiffType.Insert),
        newDiff(113, 7, DiffType.Insert),
        newDiff(144, 7, DiffType.Insert),
        newDiff(164, 7, DiffType.Insert),
        newDiff(200, 16, DiffType.Insert),
        newDiff(222, 17, DiffType.Insert),
        newDiff(245, 20, DiffType.Insert),
        newDiff(271, 19, DiffType.Insert),
        newDiff(296, 25, DiffType.Insert),
        newDiff(327, 14, DiffType.Insert),
      ],
      inlines: [
        newDiff(22, 1, DiffType.Insert),
        newDiff(44, 1, DiffType.Insert),
        newDiff(67, 1, DiffType.Insert),
        newDiff(93, 1, DiffType.Insert),
        newDiff(118, 1, DiffType.Insert),
        newDiff(149, 1, DiffType.Insert),
        newDiff(169, 1, DiffType.Insert),
      ],
    });
  });

  it('large object compare keeps exact mixed hunk and inline detail', async () => {
    const left = `{
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

    const result = await diffStructured('json', left, right);

    expectStructured(result, {
      hunks: [
        newDiff(55, 19, DiffType.Delete),
        newDiff(158, 1, DiffType.Delete),
        newDiff(181, 4, DiffType.Delete),
        newDiff(206, 4, DiffType.Delete),
        newDiff(252, 4, DiffType.Delete),
        newDiff(282, 12, DiffType.Delete),
        newDiff(354, 72, DiffType.Delete),
        newDiff(440, 72, DiffType.Delete),
        newDiff(526, 72, DiffType.Delete),
        newDiff(612, 72, DiffType.Delete),
        newDiff(718, 64, DiffType.Delete),
        newDiff(838, 12, DiffType.Delete),
        newDiff(927, 78, DiffType.Delete),
        newDiff(1011, 21, DiffType.Delete),
        newDiff(55, 17, DiffType.Insert),
        newDiff(156, 3, DiffType.Insert),
        newDiff(169, 13, DiffType.Insert),
        newDiff(204, 6, DiffType.Insert),
        newDiff(231, 5, DiffType.Insert),
        newDiff(278, 2, DiffType.Insert),
        newDiff(306, 4, DiffType.Insert),
        newDiff(390, 50, DiffType.Insert),
        newDiff(476, 12, DiffType.Insert),
        newDiff(518, 14, DiffType.Insert),
        newDiff(542, 4, DiffType.Insert),
        newDiff(623, 78, DiffType.Insert),
        newDiff(707, 78, DiffType.Insert),
      ],
      inlines: [newDiff(69, 1, DiffType.Delete), newDiff(71, 3, DiffType.Delete), newDiff(69, 2, DiffType.Insert)],
    });
  });

  it('nested array/object compare keeps both hunk and inline detail', async () => {
    const left = `[
    {
      "foo": 1,
      "bar": "baz",
      "values": [
        "1777777777777777"
      ]
    },
    {
      "foo": 9,
      "bar": "qux",
      "values": [
        "1690848000000",
        "1691193600000"
      ]
    }
]`;
    const right = `[
    {
      "foo": 7,
      "bar": "baz",
      "values": [
        "1777777777777777"
      ]
    },
    {
      "foo": 9,
      "bar": "qux",
      "values": [
        "0xc000c6e720",
        "0xc000c6e728"
      ]
    }
]`;

    const result = await diffStructured('json', left, right);

    expectStructured(result, {
      hunks: [
        newDiff(21, 1, DiffType.Delete),
        newDiff(172, 15, DiffType.Delete),
        newDiff(197, 15, DiffType.Delete),
        newDiff(21, 1, DiffType.Insert),
        newDiff(172, 14, DiffType.Insert),
        newDiff(196, 14, DiffType.Insert),
      ],
      inlines: [
        newDiff(21, 1, DiffType.Delete),
        newDiff(173, 12, DiffType.Delete),
        newDiff(198, 13, DiffType.Delete),
        newDiff(21, 1, DiffType.Insert),
        newDiff(173, 11, DiffType.Insert),
        newDiff(197, 12, DiffType.Insert),
      ],
    });
  });

  it('array element removal keeps the exact object span', async () => {
    const left = `[
    {
        "OBJ_ID": "CN=Kate Smith,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
        "userAccountControl": "512",
        "objectGUID": "b3067a77-875b-4208-9ee3-39128adeb654",
        "lastLogon": "0",
        "sAMAccountName": "ksmith",
        "userPrincipalName": "ksmith@cloudaddc.qalab.cam.novell.com",
        "distinguishedName": "CN=Kate Smith,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
    },
    {
        "OBJ_ID": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
        "userAccountControl": "512",
        "objectGUID": "c3f7dae9-9b4f-4d55-a1ec-bf9ef45061c3",
        "lastLogon": "130766915788304915",
        "sAMAccountName": "tswan",
        "userPrincipalName": "tswan@cloudaddc.qalab.cam.novell.com",
        "distinguishedName": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
    }
]`;
    const right = `[
    {
        "OBJ_ID": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com",
        "userAccountControl": "512",
        "objectGUID": "c3f7dae9-9b4f-4d55-a1ec-bf9ef45061c3",
        "lastLogon": "130766915788304915",
        "sAMAccountName": "tswan",
        "userPrincipalName": "tswan@cloudaddc.qalab.cam.novell.com",
        "distinguishedName": "CN=Timothy Swan,OU=Users,OU=Willow,DC=cloudaddc,DC=qalab,DC=cam,DC=novell,DC=com"
    }
]`;

    const result = await diffStructured('json', left, right);

    expectStructured(result, {
      hunks: [newDiff(6, 448, DiffType.Delete)],
    });
  });
});
