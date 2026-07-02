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

type CliGraphMetadataPayload = {
  source_label: string;
  expression: string;
  language: string;
  source_url: string;
  byte_length?: number;
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

  const metadataResponse = await fetcher(`/cli/meta?token=${encodeURIComponent(token)}`);
  if (!metadataResponse.ok) {
    throw new Error(`Failed to load CLI graph metadata: HTTP ${metadataResponse.status}`);
  }

  const metadata = normalizeCliGraphMetadata(await metadataResponse.json());
  const sourceResponse = await fetcher(metadata.sourceUrl);
  if (!sourceResponse.ok) {
    throw new Error(`Failed to load CLI graph source: HTTP ${sourceResponse.status}`);
  }

  return {
    sourceLabel: metadata.sourceLabel,
    expression: metadata.expression,
    language: metadata.language,
    text: await sourceResponse.text(),
  };
}

export function normalizeCliGraphMetadata(raw: unknown): Omit<CliGraphResult, 'text'> & { sourceUrl: string } {
  if (!isCliGraphMetadataPayload(raw)) {
    throw new Error('Invalid CLI graph metadata payload');
  }

  return {
    sourceLabel: raw.source_label,
    expression: raw.expression,
    language: raw.language,
    sourceUrl: raw.source_url,
  };
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

function isCliGraphMetadataPayload(raw: unknown): raw is CliGraphMetadataPayload {
  if (!raw || typeof raw !== 'object') {
    return false;
  }

  const payload = raw as Record<string, unknown>;
  return (
    typeof payload.source_label === 'string' &&
    typeof payload.expression === 'string' &&
    typeof payload.language === 'string' &&
    typeof payload.source_url === 'string' &&
    (typeof payload.byte_length === 'number' || typeof payload.byte_length === 'undefined')
  );
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
