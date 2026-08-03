import type { Previewer } from './types';
import { buildTable } from './utils';

const dateRe = /^(20\d{2})-(\d{2})-(\d{2})(?=$|T)/;
const utcDateTimeRe = /^20\d{2}-\d{2}-\d{2}T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\dZ$/;
const offsetDateTimeRe = /^20\d{2}-\d{2}-\d{2}T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d[+-](?:[01]\d|2[0-3]):[0-5]\d$/;
const timeZoneRe = /([+-])([0-2]\d):([0-5]\d)$/;

function isTimestamp(value: string): boolean {
  return /^1(\d{9}|\d{12})$/.test(value);
}

function isValidDate(date: Date): boolean {
  return Number.isFinite(date.getTime());
}

function isValidCalendarDate(value: string): boolean {
  const match = value.match(dateRe);
  if (!match) return false;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (month < 1 || month > 12 || day < 1 || day > 31) return false;
  const date = new Date(Date.UTC(year, month - 1, day));
  return date.getUTCFullYear() === year
    && date.getUTCMonth() === month - 1
    && date.getUTCDate() === day;
}

function isDate(value: string): boolean {
  if (!isValidCalendarDate(value)) return false;
  return isValidDate(genDate(value));
}

function genDate(value: string): Date {
  if (isTimestamp(value)) {
    const numeric = Number(value);
    return new Date(numeric * (value.length === 10 ? 1000 : 1));
  }
  const date = new Date();
  const localOffsetMs = date.getTimezoneOffset() * 60 * 1000;
  if (utcDateTimeRe.test(value)) {
    const next = new Date(value.replace(/Z$/, ''));
    return new Date(next.getTime() + localOffsetMs);
  }
  if (offsetDateTimeRe.test(value)) {
    const timePart = value.slice(0, -6);
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
    if (!isValidDate(next)) return null;
    const diffMs = Date.now() - next.getTime();
    return buildTable({
      ISO: next.toISOString(),
      Local: next.toLocaleString(),
      Timestamp: String(Math.floor(next.getTime() / 1000)),
      RelativeTime: diffMs > 0 ? `${formatDuration(diffMs)} ago` : `in ${formatDuration(diffMs)}`,
    });
  },
};
