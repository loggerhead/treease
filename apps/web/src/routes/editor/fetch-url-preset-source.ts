import { findFormatByExtension } from '../../lib/import/resolve-import-source';
import type { SupportedEditorLanguageId } from '../../lib/monaco/language-support';

const MAX_URL_PRESET_SOURCE_BYTES = 1024 * 1024;

export type UrlPresetSource = {
  text: string;
  inferredLanguage: SupportedEditorLanguageId | null;
};

export async function fetchUrlPresetSource(
  rawUrl: string,
  fetcher: typeof fetch = fetch,
): Promise<UrlPresetSource> {
  let resolvedUrl: URL;
  try {
    resolvedUrl = new URL(rawUrl, window.location.href);
  } catch {
    throw new Error(`Invalid URL preset source: ${rawUrl}`);
  }

  let response: Response;
  try {
    response = await fetcher(resolvedUrl.toString());
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to fetch ${resolvedUrl.toString()}: ${message}`);
  }

  if (!response.ok) {
    throw new Error(`Failed to fetch ${resolvedUrl.toString()}: HTTP ${response.status}`);
  }

  const contentLengthHeader = response.headers.get('content-length');
  if (contentLengthHeader) {
    const contentLength = Number.parseInt(contentLengthHeader, 10);
    if (Number.isFinite(contentLength) && contentLength > MAX_URL_PRESET_SOURCE_BYTES) {
      throw new Error(
        `URL preset source exceeds ${MAX_URL_PRESET_SOURCE_BYTES} bytes: ${resolvedUrl.toString()}`,
      );
    }
  }

  const text = await response.text();
  const byteLength = new TextEncoder().encode(text).length;
  if (byteLength > MAX_URL_PRESET_SOURCE_BYTES) {
    throw new Error(`URL preset source exceeds ${MAX_URL_PRESET_SOURCE_BYTES} bytes: ${resolvedUrl.toString()}`);
  }

  const inferredLanguage = findFormatByExtension(resolvedUrl.pathname) as SupportedEditorLanguageId | null;
  return { text, inferredLanguage };
}
