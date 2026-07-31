import { normalizeEditorSplitRatio } from './editor-layout-state';

export const EDITOR_SPLIT_RATIO_COOKIE = 'treease_editor_split_ratio';
const COOKIE_MAX_AGE_SECONDS = 60 * 60 * 24 * 365;

export function readEditorSplitRatioCookie(value: string | undefined): number | null {
  if (!value || !/^(?:0|1)\.\d+$/.test(value)) return null;
  return normalizeEditorSplitRatio(Number(value));
}

export function writeEditorSplitRatioCookie(splitRatio: number): void {
  const normalized = normalizeEditorSplitRatio(splitRatio);
  if (normalized === null || typeof document === 'undefined') return;
  const secure = window.location.protocol === 'https:' ? '; Secure' : '';
  document.cookie = `${EDITOR_SPLIT_RATIO_COOKIE}=${normalized}; Path=/; Max-Age=${COOKIE_MAX_AGE_SECONDS}; SameSite=Lax${secure}`;
}

export function clearEditorSplitRatioCookie(): void {
  if (typeof document === 'undefined') return;
  const secure = window.location.protocol === 'https:' ? '; Secure' : '';
  document.cookie = `${EDITOR_SPLIT_RATIO_COOKIE}=; Path=/; Max-Age=0; SameSite=Lax${secure}`;
}
