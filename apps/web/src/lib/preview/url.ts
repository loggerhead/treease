import type { Previewer } from './types';
import { buildTable, joinSections, wrapHeading } from './utils';

function isUri(value: string): boolean {
  return typeof value === 'string' && /^[a-z]+[a-z0-9+.-]*:\/\//.test(value);
}

function mapToRecord(map: Map<string, string | Map<string, string | Map<string, any>>>): Record<string, string | Record<string, any>> {
  const result: Record<string, string | Record<string, any>> = {};
  for (const [key, current] of map) {
    if (current instanceof Map) {
      result[key] = mapToRecord(current);
    } else {
      result[key] = current;
    }
  }
  return result;
}

function urlToMap(value: string, maxLevel?: number): Map<string, string | Map<string, any>> {
  const fullUri = /^https?:\/\//.test(value);
  const url = new URL(fullUri ? value : `http://treease.local/${value.replace(/^\//, '')}`);
  const result = new Map<string, string | Map<string, any>>();
  if (fullUri) {
    if (url.protocol) result.set('Protocol', url.protocol.replace(/:$/, ''));
    if (url.hostname) result.set('Host', url.hostname);
  }
  if (url.username) result.set('Username', url.username);
  if (url.password) result.set('Password', url.password);
  if (url.port) result.set('Port', url.port);
  if (url.pathname) result.set('Path', url.pathname);
  if (url.hash) result.set('Hash', url.hash);
  if (maxLevel === undefined || maxLevel > 0) {
    const query = new Map<string, string | Map<string, any> | Array<string | Map<string, any>>>();
    const duplicates = new Map<string, number>();
    url.searchParams.forEach((_, name) => {
      duplicates.set(name, (duplicates.get(name) ?? 0) + 1);
    });
    url.searchParams.forEach((nextValue, name) => {
      const level = maxLevel === undefined ? undefined : maxLevel - 1;
      const resolved = level !== undefined && level > 0 && isUri(nextValue) ? urlToMap(nextValue, level) : nextValue;
      if ((duplicates.get(name) ?? 0) > 1) {
        const current = (query.get(name) ?? []) as Array<string | Map<string, any>>;
        current.push(resolved);
        query.set(name, current);
      } else {
        query.set(name, resolved);
      }
    });
    if (query.size > 0) {
      result.set('Query', query as Map<string, any>);
    }
  } else if (url.searchParams.size > 0) {
    result.set('Query', url.searchParams.toString());
  }
  return result;
}

export const urlPreviewer: Previewer = {
  detector: ({ value }) => /^https?:\/\/.*/.test(value) || isUri(value),
  generator: ({ value }) => {
    const { Query: query, ...rest } = mapToRecord(urlToMap(value, 1));
    const sections = [buildTable(rest as Record<string, string>)];
    if (query && typeof query === 'object') {
      const queryRecord = Object.fromEntries(
        Object.entries(query as Record<string, unknown>).map(([key, current]) => [
          key,
          typeof current === 'string' ? current : JSON.stringify(current),
        ]),
      );
      sections.push(...joinSections([wrapHeading('Query'), buildTable(queryRecord)]));
    }
    return sections;
  },
};
