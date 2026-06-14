export function escapeHtml(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

export function wrapPre(text: string): string {
  return `<pre>${escapeHtml(text)}</pre>`;
}

export function wrapHeading(title: string): string {
  return `<div><strong>${escapeHtml(title)}</strong></div>`;
}

export function joinSections(parts: Array<string | null | undefined>): string[] {
  return parts.filter((part): part is string => typeof part === 'string' && part.length > 0);
}

export function buildTable(
  data: Record<string, string>,
  styleFn?: (key: string, value: string) => { keyStyle?: string; valueStyle?: string },
): string {
  const rows = Object.entries(data)
    .map(([key, value]) => {
      const { keyStyle = '', valueStyle = '' } = styleFn ? styleFn(key, value) : {};
      const keyAttr = keyStyle ? ` style="${escapeHtml(keyStyle)}"` : '';
      const valueAttr = valueStyle ? ` style="${escapeHtml(valueStyle)}"` : '';
      return `<tr><td><strong${keyAttr}>${escapeHtml(key)}</strong></td><td> \t　</td><td${valueAttr}>${escapeHtml(value)}</td></tr>`;
    })
    .join('');
  return `<table>${rows}</table>`;
}
