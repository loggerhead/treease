import { describe, expect, it } from 'vitest';
import { buildDiffPlans, type DiffPair } from './diff-plan';

const monaco = {
  editor: {
    MinimapPosition: { Inline: 1 },
    OverviewRulerLane: { Full: 2 },
  },
};

describe('diff-plan', () => {
  describe('basic results', () => {
    it('returns empty decorations for empty pairs', () => {
      const result = buildDiffPlans(monaco, [], '', '');
      expect(result.left.decorations).toEqual([]);
      expect(result.right.decorations).toEqual([]);
      expect(result.left.fillRanges).toEqual([]);
      expect(result.right.fillRanges).toEqual([]);
    });

    it('sets firstLine on left/right', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: { byteOffset: 0, byteLength: 3, type: 1, inlineDiffs: [] },
          hasRight: 0,
          right: { byteOffset: 0, byteLength: 0, type: 0, inlineDiffs: [] },
        },
      ];
      const result = buildDiffPlans(monaco, pairs, 'abc', '');
      expect(result.left.firstLine).toBe(1);
      expect(result.right.firstLine).toBeUndefined();
    });
  });

  describe('inline diffs', () => {
    it('handles inline diff ranges', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: {
            byteOffset: 0,
            byteLength: 1,
            type: 1,
            inlineDiffs: [{ byteOffset: 0, byteLength: 1, type: 1, inlineDiffs: [] }],
          },
          hasRight: 1,
          right: {
            byteOffset: 0,
            byteLength: 1,
            type: 0,
            inlineDiffs: [{ byteOffset: 0, byteLength: 1, type: 0, inlineDiffs: [] }],
          },
        },
      ];
      const result = buildDiffPlans(monaco, pairs, 'a', 'b');
      expect(result.left.decorations.length).toBeGreaterThan(0);
      expect(result.right.decorations.length).toBeGreaterThan(0);
    });

    it('keeps single-char inline delete visible for 0.125 -> 0.15', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: {
            byteOffset: 8,
            byteLength: 5,
            type: 1,
            inlineDiffs: [{ byteOffset: 11, byteLength: 1, type: 1, inlineDiffs: [] }],
          },
          hasRight: 1,
          right: {
            byteOffset: 8,
            byteLength: 4,
            type: 0,
            inlineDiffs: [],
          },
        },
      ];

      const result = buildDiffPlans(monaco, pairs, '{"ratio":0.125}', '{"ratio":0.15}');
      const inline = result.left.decorations.find((d) => d.options.inlineClassName === 'diff-inline-del');

      expect(inline).not.toBeUndefined();
      expect(inline!.options.inlineClassName).toBe('diff-inline-del');
      expect(inline!.range.startLineNumber).toBe(1);
      expect(inline?.range.startColumn).toBe(12);
      expect(inline?.range.endColumn).toBe(13);
    });
  });

  describe('multi-line hunks', () => {
    it('produces left-only deletion decorations on correct lines', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: { byteOffset: 0, byteLength: 12, type: 1, inlineDiffs: [] },
          hasRight: 0,
          right: { byteOffset: 0, byteLength: 0, type: 0, inlineDiffs: [] },
        },
      ];
      const result = buildDiffPlans(monaco, pairs, 'line1\nline2\n', '');
      expect(result.left.firstLine).toBe(1);
      expect(result.right.firstLine).toBeUndefined();
      const hunkDecos = result.left.decorations.filter((d) => d.options.isWholeLine);
      expect(hunkDecos.length).toBeGreaterThan(0);
      hunkDecos.forEach((d) => {
        expect(d.options.className).toContain('del');
      });
      expect(hunkDecos[0].range.startLineNumber).toBe(1);
    });

    it('keeps a single newline deletion on the same line', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: { byteOffset: 2, byteLength: 1, type: 1, inlineDiffs: [] },
          hasRight: 0,
          right: { byteOffset: 0, byteLength: 0, type: 0, inlineDiffs: [] },
        },
      ];
      const result = buildDiffPlans(monaco, pairs, '{\n\n  return tokens;\n}', '{\n  return tokens;\n}');
      const leftHunk = result.left.decorations.find((d) => d.options.isWholeLine);
      expect(leftHunk?.range.startLineNumber).toBe(2);
      expect(leftHunk?.range.endLineNumber).toBe(2);
    });

    it('produces right-only insertion decorations with ins class', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 0,
          left: { byteOffset: 0, byteLength: 0, type: 0, inlineDiffs: [] },
          hasRight: 1,
          right: { byteOffset: 0, byteLength: 10, type: 0, inlineDiffs: [] },
        },
      ];
      const result = buildDiffPlans(monaco, pairs, '', 'new1\nnew2\n');
      expect(result.right.firstLine).toBe(1);
      const hunkDecos = result.right.decorations.filter((d) => d.options.isWholeLine);
      expect(hunkDecos.length).toBeGreaterThan(0);
      hunkDecos.forEach((d) => {
        expect(d.options.className).toContain('ins');
      });
    });
  });

  describe('fill ranges and deletion-only cases', () => {
    it('produces fillRanges for line alignment', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: { byteOffset: 0, byteLength: 3, type: 1, inlineDiffs: [] },
          hasRight: 0,
          right: { byteOffset: 0, byteLength: 0, type: 0, inlineDiffs: [] },
        },
      ];
      const result = buildDiffPlans(monaco, pairs, 'abc', '');
      expect(result.right.fillRanges.length).toBeGreaterThan(0);
      const fill = result.right.fillRanges[0];
      expect(fill.startLineNumber).toBeGreaterThanOrEqual(1);
      expect(fill.endLineNumber).toBeGreaterThanOrEqual(fill.startLineNumber);
    });

    it('keeps fill ranges anchored to local hunk gaps across multiple pairs', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: { byteOffset: 0, byteLength: 17, type: 1, inlineDiffs: [] },
          hasRight: 1,
          right: { byteOffset: 0, byteLength: 9, type: 0, inlineDiffs: [] },
        },
        {
          hasLeft: 1,
          left: { byteOffset: 18, byteLength: 5, type: 1, inlineDiffs: [] },
          hasRight: 1,
          right: { byteOffset: 10, byteLength: 23, type: 0, inlineDiffs: [] },
        },
      ];

      const leftText = 'x\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx\nx';
      const rightText = 'y\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny';

      const result = buildDiffPlans(monaco, pairs, leftText, rightText);

      expect(result.right.fillRanges).toEqual([{ startLineNumber: 6, endLineNumber: 9 }]);
      expect(result.left.fillRanges).toEqual([{ startLineNumber: 13, endLineNumber: 21 }]);
    });

    it('does not produce add decorations on left for deletion-only pairs', () => {
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: { byteOffset: 0, byteLength: 5, type: 1, inlineDiffs: [] },
          hasRight: 0,
          right: { byteOffset: 0, byteLength: 0, type: 0, inlineDiffs: [] },
        },
      ];
      const result = buildDiffPlans(monaco, pairs, 'hello', '');
      const addDecos = result.left.decorations.filter(
        (d) => d.options.className && d.options.className.includes('ins'),
      );
      expect(addDecos).toHaveLength(0);
    });

    it('skips tail fill on left for an internal right-only pair but keeps later anchors stable', () => {
      const leftText = [
        '{',
        '  "title": "Example",',
        '  "count": 42,',
        '  "ratio": 0.125,',
        '  "active": true,',
        '  "nothing": null,',
        '  "tags": ["alpha", "beta", "gamma"],',
        '  "meta": {',
        '    "nested": {',
        '      "id": "item-001",',
        '      "flags": [true, false, true],',
        '      "scores": [1, 2, 3, 4],',
        '      "profile": {',
        '        "name": "Alice",',
        '        "age": 30,',
        '        "contact": {',
        '          "email": "alice@example.com",',
        '          "phones": ["+1-555-0100", "+1-555-0101"]',
        '        }',
        '      }',
        '    }',
        '  }',
        '}',
      ].join('\n');
      const rightText = [
        '{',
        '  "BaseResp": {',
        '    "StatusMessage": "success",',
        '    "StatusCode": 0',
        '  },',
        '  "tax_rates": [',
        '    {',
        '      "account_id": 9999999999999999999,',
        '      "sum_tax_rate": 0.98765,',
        '      "tax_rates": [',
        '        {',
        '          "tax_category": "QST",',
        '          "tax_rate": 0.98765,',
        '          "tax_serial": ""',
        '        }',
        '      ],',
        '      "tax_status": 1,',
        '      "valid_time": "2024-12-30 00:00:00",',
        '      "invalid_time": ""',
        '    }',
        '  ],',
        '  "total": 1',
        '}',
      ].join('\n');
      const pairs: DiffPair[] = [
        {
          hasLeft: 1,
          left: {
            byteOffset: 2,
            byteLength: 422,
            type: 1,
            inlineDiffs: [{ byteOffset: 5, byteLength: 419, type: 1, inlineDiffs: [] }],
          },
          hasRight: 1,
          right: {
            byteOffset: 2,
            byteLength: 289,
            type: 0,
            inlineDiffs: [{ byteOffset: 5, byteLength: 286, type: 0, inlineDiffs: [] }],
          },
        },
        {
          hasLeft: 0,
          left: { byteOffset: 0, byteLength: 0, type: 1, inlineDiffs: [] },
          hasRight: 1,
          right: { byteOffset: 302, byteLength: 99, type: 0, inlineDiffs: [] },
        },
        {
          hasLeft: 1,
          left: {
            byteOffset: 443,
            byteLength: 9,
            type: 1,
            inlineDiffs: [
              { byteOffset: 445, byteLength: 3, type: 1, inlineDiffs: [] },
              { byteOffset: 451, byteLength: 1, type: 1, inlineDiffs: [] },
            ],
          },
          hasRight: 1,
          right: {
            byteOffset: 408,
            byteLength: 17,
            type: 0,
            inlineDiffs: [
              { byteOffset: 410, byteLength: 2, type: 0, inlineDiffs: [] },
              { byteOffset: 415, byteLength: 10, type: 0, inlineDiffs: [] },
            ],
          },
        },
      ];

      const result = buildDiffPlans(monaco, pairs, leftText, rightText);
      const leftHunks = result.left.decorations.filter((d) => d.options.isWholeLine);
      const lastLeftHunk = leftHunks[leftHunks.length - 1];

      expect(lastLeftHunk?.range.startLineNumber).toBe(21);
      expect(lastLeftHunk?.range.endLineNumber).toBe(22);
      expect(result.left.fillRanges).toEqual([]);
      expect(result.right.fillRanges).toEqual([{ startLineNumber: 15, endLineNumber: 18 }]);
    });
  });
});
