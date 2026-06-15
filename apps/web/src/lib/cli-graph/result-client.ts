export type CliGraphResult = {
  sourceLabel: string;
  expression: string;
  language: string;
  text: string;
};

type CliGraphResultPayload = {
  source_label: string;
  expression: string;
  language: string;
  text: string;
};

type CliGraphResultFetcher = (input: string) => Promise<Response>;

export function readCliGraphTokenFromSearch(search: string): string {
  return new URLSearchParams(search).get('token') ?? '';
}

export async function fetchCliGraphResult(
  token: string,
  fetcher: CliGraphResultFetcher = fetch,
): Promise<CliGraphResult> {
  if (!token) {
    throw new Error('Missing CLI graph token');
  }

  const response = await fetcher(`/cli/result?token=${encodeURIComponent(token)}`);
  if (!response.ok) {
    throw new Error(`Failed to load CLI graph result: HTTP ${response.status}`);
  }

  return normalizeCliGraphResult(await response.json());
}

export function normalizeCliGraphResult(raw: unknown): CliGraphResult {
  if (!isCliGraphResultPayload(raw)) {
    throw new Error('Invalid CLI graph result payload');
  }

  return {
    sourceLabel: raw.source_label,
    expression: raw.expression,
    language: raw.language,
    text: raw.text,
  };
}

function isCliGraphResultPayload(raw: unknown): raw is CliGraphResultPayload {
  if (!raw || typeof raw !== 'object') {
    return false;
  }

  const payload = raw as Record<string, unknown>;
  return (
    typeof payload.source_label === 'string' &&
    typeof payload.expression === 'string' &&
    typeof payload.language === 'string' &&
    typeof payload.text === 'string'
  );
}
