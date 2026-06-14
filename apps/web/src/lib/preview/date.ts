import type { Previewer } from './types';
import { buildTable } from './utils';

const dateRe = /20\d{2}-[01]\d-[0-3]\d/;
const timeRe = /[0-2]\d:[0-5]\d:[0-5]\d/;
const timeZoneRe = /([+-])([0-2]\d):([0-5]\d)/;

function concatRe(re1: RegExp | string, ...rest: Array<RegExp | string>) {
  const first = typeof re1 === 'string' ? re1 : re1.source;
  return new RegExp([first, ...rest.map((item) => (typeof item === 'string' ? item : item.source))].join(''));
}

function isTimestamp(value: string): boolean {
  return /^1(\d{9}|\d{12})$/.test(value);
}

function isDate(value: string): boolean {
  return concatRe('^', dateRe).test(value);
}

function genDate(value: string): Date {
  if (isTimestamp(value)) {
    const numeric = Number(value);
    return new Date(numeric * (value.length === 10 ? 1000 : 1));
  }
  const date = new Date();
  const localOffsetMs = date.getTimezoneOffset() * 60 * 1000;
  if (concatRe('^', dateRe, 'T', timeRe, 'Z').test(value)) {
    const next = new Date(value.replace(/Z$/, ''));
    return new Date(next.getTime() + localOffsetMs);
  }
  if (concatRe('^', dateRe, 'T', timeRe, timeZoneRe, '$').test(value)) {
    const timePart = value.replace(concatRe(timeZoneRe, '$'), '');
    const utc = new Date(`${timePart}Z`);
    const match = value.match(timeZoneRe);
    if (!match) return utc;
    const sign = match[1];
    const hours = Number(match[2]);
    const minutes = Number(match[3]);
    const offset = (hours * 60 + minutes) * 60 * 1000 * (sign === '+' ? -1 : 1);
    return new Date(utc.getTime() + offset);
  }
  return new Date(value);
}

function formatDuration(diffMs: number): string {
  const abs = Math.abs(diffMs);
  const seconds = Math.floor(abs / 1000) % 60;
  const minutes = Math.floor(abs / (1000 * 60)) % 60;
  const hours = Math.floor(abs / (1000 * 60 * 60)) % 24;
  const days = Math.floor(abs / (1000 * 60 * 60 * 24));
  let result = '';
  if (days > 0) result += `${days}d`;
  if (hours > 0) result += `${hours}h`;
  if (minutes > 0) result += `${minutes}m`;
  if (seconds > 0) result += `${seconds}s`;
  return result || '0s';
}

export const datePreviewer: Previewer = {
  detector: ({ value }) => {
    if (isTimestamp(value)) {
      const timestamp = Number(value) * (value.length === 10 ? 1000 : 1);
      return new Date(timestamp).getTime() === timestamp;
    }
    return isDate(value);
  },
  generator: ({ value }) => {
    const next = genDate(value);
    const diffMs = Date.now() - next.getTime();
    return buildTable({
      ISO: next.toISOString(),
      Local: next.toLocaleString(),
      Timestamp: String(Math.floor(next.getTime() / 1000)),
      RelativeTime: diffMs > 0 ? `${formatDuration(diffMs)} ago` : `in ${formatDuration(diffMs)}`,
    });
  },
};
