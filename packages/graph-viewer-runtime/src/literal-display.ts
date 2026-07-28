export function formatScalarLiteral(text: string, valueType: string, language?: string): string {
  if (valueType === 'string' && text === '') return '""';
  if (valueType === 'null' && text === '') return language === 'python' ? 'None' : 'null';
  if (language !== 'python') return text;
  if (valueType === 'boolean') {
    if (text === 'true') return 'True';
    if (text === 'false') return 'False';
  }
  if (valueType === 'null' && text === 'null') return 'None';
  return text;
}

export function resolveGraphCellDisplayText(
  text: string | null | undefined,
  value: string | null | undefined,
  valueType: string,
  language?: string,
): string {
  const raw = text === '' || text == null ? (value ?? '') : text;
  return formatScalarLiteral(String(raw), valueType, language);
}

function formatPythonValue(value: unknown, depth: number): string {
  if (value === null) return 'None';
  if (typeof value === 'boolean') return value ? 'True' : 'False';
  if (typeof value === 'number') return Number.isFinite(value) ? String(value) : JSON.stringify(value);
  if (typeof value === 'string') return JSON.stringify(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return '[]';
    const indent = '  '.repeat(depth);
    const childIndent = '  '.repeat(depth + 1);
    const items = value.map((item) => `${childIndent}${formatPythonValue(item, depth + 1)}`);
    return `[
${items.join(',\n')}
${indent}]`;
  }
  if (value && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length === 0) return '{}';
    const indent = '  '.repeat(depth);
    const childIndent = '  '.repeat(depth + 1);
    const lines = entries.map(([key, item]) => `${childIndent}${JSON.stringify(key)}: ${formatPythonValue(item, depth + 1)}`);
    return `{
${lines.join(',\n')}
${indent}}`;
  }
  return JSON.stringify(value);
}

export function formatStructuredPreview(value: unknown, language?: string): string {
  if (language === 'python') {
    return formatPythonValue(value, 0);
  }
  return JSON.stringify(value, null, 2);
}
