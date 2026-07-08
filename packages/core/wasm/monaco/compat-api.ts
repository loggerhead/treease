import type { DiffResult, FormatOptions, ParseOptions, WasmError } from './types';
import type { PathSeg, TreeNode } from '../serde-types';
import { callWasm } from './shared-api';

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export async function parseValueToTree(input: ParseValueToTreeJsonInput): Promise<ParseValueToTreeJsonOutput> {
  return callWasm((mod) => mod.parse_value_to_tree(input as any));
}

export async function formatText(language: string, text: string, options?: FormatOptions): Promise<string> {
  return callWasm((mod) =>
    mod.format_text({
      language,
      text,
      indent: options?.indent ?? null,
      nest: options?.nest ?? null,
      sortKeys: options?.sortKeys ?? null,
    } as any),
  );
}

export async function minifyText(language: string, text: string, _options?: FormatOptions): Promise<string> {
  return callWasm((mod) => mod.minify_text({ language, text } as any));
}

export async function convertText(
  sourceLanguage: string,
  targetFormat: string,
  text: string,
  options?: FormatOptions,
): Promise<string> {
  return callWasm((mod) =>
    mod.convert_text({ sourceLanguage, targetFormat, text, indent: options?.indent ?? null } as any),
  );
}

export async function getDiagnostics(language: string, text: string): Promise<WasmError[]> {
  const result = await callWasm((mod) => (mod as any).get_diagnostics({ language, text } as any));
  return decodeWasmErrors(result?.diagnostics ?? []);
}

export async function parseToTree(language: string, text: string, _options?: ParseOptions): Promise<TreeNode> {
  const result = await callWasm((mod) => mod.parse_value_to_tree({ language, text, nest: false } as any));
  if (result && result.tree) {
    return result.tree as unknown as TreeNode;
  }
  throw new Error('parseToTree failed: no tree in parse_value_to_tree output');
}

export async function parseValueForPath(
  language: string,
  _documentKey: string,
  _text: string,
  _path: PathSeg[],
  rawEdit: string,
  _preferKey: boolean,
  _options?: ParseOptions,
): Promise<TreeNode> {
  try {
    return await parseToTree(language, rawEdit);
  } catch {
    return parseToTree(language, JSON.stringify(rawEdit));
  }
}

export async function findJsonBlockAtPosition(
  language: string,
  text: string,
  row: number,
  column: number,
): Promise<any> {
  const result = await callWasm((mod) => mod.find_json_block_at_position_wasm({ language, text, row, column } as any));
  if (!result) return { found: false, startByte: 0, endByte: 0, startRow: 0, startColumn: 0, endRow: 0, endColumn: 0 };
  return {
    found: result.found,
    startByte: result.startByte,
    endByte: result.endByte,
    startRow: result.startRow,
    startColumn: result.startColumn,
    endRow: result.endRow,
    endColumn: result.endColumn,
  };
}


export async function applyValueEdit(
  language: string,
  text: string,
  path: PathSeg[],
  preferKey: boolean,
  value: unknown,
): Promise<string> {
  try {
    const result = await callWasm((mod) =>
      mod.apply_value_edit_wasm({ language, text, path, preferKey, value: JSON.stringify(value) } as any),
    );
    return result ?? text;
  } catch (error) {
    throw new Error('applyValueEdit failed: ' + (error instanceof Error ? error.message : String(error)));
  }
}
export type ApplyValueEditCanonicalResult = {
  text: string;
  tree: TreeNode | null;
  value: unknown;
};
export async function applyValueEditCanonical(
  language: string,
  text: string,
  path: PathSeg[],
  preferKey: boolean,
  value: unknown,
): Promise<ApplyValueEditCanonicalResult> {
  try {
    return await callWasm((mod) =>
      mod.apply_value_edit_canonical_wasm({ language, text, path, preferKey, value: JSON.stringify(value) } as any),
    ) as ApplyValueEditCanonicalResult;
  } catch (error) {
    throw new Error('applyValueEditCanonical failed: ' + (error instanceof Error ? error.message : String(error)));
  }
}
export async function isStructurallyEqual(language: string, left: string, right: string): Promise<boolean> {
  return callWasm((mod) => mod.compare_structured_wasm({ language, left, right } as any));
}

// Backward-compatible alias. Prefer `isStructurallyEqual` for new call sites.
export async function compareStructured(language: string, left: string, right: string): Promise<boolean> {
  return isStructurallyEqual(language, left, right);
}

export async function diffStructured(language: string, left: string, right: string): Promise<DiffResult> {
  return callWasm((mod) => mod.diff_structured_wasm({ language, left, right } as any)) as Promise<DiffResult>;
}

export async function diffText(left: string, right: string): Promise<DiffResult> {
  return callWasm((mod) => mod.diff_text_wasm({ left, right } as any)) as Promise<DiffResult>;
}

export async function runYqText(
  language: string,
  text: string,
  expression: string,
  _options?: FormatOptions,
): Promise<string> {
  return callWasm((mod) => mod.run_yq_text_wasm({ language, text, expression } as any));
}

export async function sortText(language: string, text: string, options?: FormatOptions): Promise<string> {
  return formatText(language, text, { ...options, sortKeys: true });
}

export type ParseValueToTreeJsonInput = {
  language: string;
  text: string;
  nest: boolean;
};

export type ParseValueToTreeJsonOutput = {
  tree: TreeNode | null;
  value: unknown;
};

export type FormatJsonInput = {
  language: string;
  text: string;
  indent?: number | null;
  nest?: boolean | null;
  sortKeys?: boolean | null;
};

export type FormatJsonOutput = {
  text: string;
};

export type MinifyJsonInput = {
  language: string;
  text: string;
};

export type ConvertJsonInput = {
  sourceLanguage: string;
  targetFormat: string;
  text: string;
  indent?: number | null;
};

export async function parseValueToTreeJson(input: ParseValueToTreeJsonInput): Promise<ParseValueToTreeJsonOutput> {
  return parseValueToTree(input);
}

export async function formatJson(input: FormatJsonInput): Promise<FormatJsonOutput> {
  const text = await formatText(input.language, input.text, {
    indent: input.indent ?? undefined,
    nest: input.nest ?? undefined,
    sortKeys: input.sortKeys ?? undefined,
  });
  return { text };
}

export async function minifyJson(input: MinifyJsonInput): Promise<FormatJsonOutput> {
  const text = await minifyText(input.language, input.text);
  return { text };
}

export async function convertJson(input: ConvertJsonInput): Promise<FormatJsonOutput> {
  const text = await convertText(input.sourceLanguage, input.targetFormat, input.text, {
    indent: input.indent ?? undefined,
  });
  return { text };
}

function decodeWasmErrors(raw: number[] | Uint32Array): WasmError[] {
  const errors: WasmError[] = [];
  const stride = 5;
  for (let index = 0; index + stride <= raw.length; index += stride) {
    errors.push({
      startLineNumber: raw[index],
      startColumn: raw[index + 1],
      endLineNumber: raw[index + 2],
      endColumn: raw[index + 3],
      kind: raw[index + 4],
    });
  }
  return errors;
}
