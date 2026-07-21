export type FeedbackConsoleLog = {
  level: 'log' | 'info' | 'warn' | 'error';
  message: string;
  timestamp: string;
};

const entries: FeedbackConsoleLog[] = [];
let installed = false;

export function installFeedbackConsoleLogBuffer(): void {
  if (installed || typeof window === 'undefined') return;
  installed = true;

  for (const level of ['log', 'info', 'warn', 'error'] as const) {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      entries.push({
        level,
        message: args.map(formatConsoleValue).join(' '),
        timestamp: new Date().toISOString(),
      });
      if (entries.length > 50) entries.splice(0, entries.length - 50);
      original(...args);
    };
  }
}

export function getFeedbackConsoleLogs(): FeedbackConsoleLog[] {
  return entries.slice();
}

function formatConsoleValue(value: unknown): string {
  if (value instanceof Error) return `${value.name}: ${value.message}`;
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
