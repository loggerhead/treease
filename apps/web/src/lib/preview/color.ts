import type { Previewer } from './types';
import { buildTable, joinSections } from './utils';

export type RgbaColor = { r: number; g: number; b: number; a: number };
export type CssColorFormat = 'hex' | 'hexa' | 'rgb' | 'rgba' | 'hsl' | 'hsla';

const hexTokenSource = '#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})\\b';
const rgbTokenSource = 'rgba?\\(\\s*\\d{1,3}\\s*,\\s*\\d{1,3}\\s*,\\s*\\d{1,3}(?:\\s*,\\s*(?:0|1|0?\\.\\d+|1\\.0+))?\\s*\\)';
const hslTokenSource = 'hsla?\\(\\s*\\d{1,3}\\s*,\\s*\\d{1,3}%\\s*,\\s*\\d{1,3}%(?:\\s*,\\s*(?:0|1|0?\\.\\d+|1\\.0+))?\\s*\\)';

const cssColorTokenSource = `(?:${hexTokenSource}|${rgbTokenSource}|${hslTokenSource})`;
const exactCssColorPattern = new RegExp(`^${cssColorTokenSource}$`, 'i');

function hexToDec(hex: string): number {
  return Number.parseInt(hex, 16);
}

function decToHex(dec: number): string {
  const hex = Math.round(dec).toString(16);
  return hex.length === 1 ? `0${hex}` : hex;
}

function clampByte(dec: number): number {
  return Math.max(0, Math.min(255, dec));
}

function clampAlpha(alpha: number): number {
  if (Number.isNaN(alpha)) return 1;
  return Math.max(0, Math.min(1, alpha));
}

function roundAlpha(alpha: number): number {
  return Math.round(clampAlpha(alpha) * 100) / 100;
}

function parseRgbChannel(value: string): number | undefined {
  const parsed = Number.parseInt(value, 10);
  if (Number.isNaN(parsed) || parsed < 0 || parsed > 255) return undefined;
  return parsed;
}

function parsePercentChannel(value: string): number | undefined {
  const parsed = Number.parseInt(value, 10);
  if (Number.isNaN(parsed) || parsed < 0 || parsed > 100) return undefined;
  return parsed;
}

function parseAlphaChannel(value?: string): number | undefined {
  if (value === undefined) return 1;
  const parsed = Number.parseFloat(value);
  if (Number.isNaN(parsed) || parsed < 0 || parsed > 1) return undefined;
  return parsed;
}

function hexToRgb(hex: string): RgbaColor | undefined {
  const match = hex.match(/^#?([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/);
  if (!match) return undefined;
  let hexStr = match[1];
  let a = 1;
  if (hexStr.length === 3 || hexStr.length === 4) {
    hexStr = hexStr
      .split('')
      .map((char) => `${char}${char}`)
      .join('');
  }
  if (hexStr.length === 8) {
    a = hexToDec(hexStr.slice(6, 8)) / 255;
    hexStr = hexStr.slice(0, 6);
  }
  return {
    r: hexToDec(hexStr.slice(0, 2)),
    g: hexToDec(hexStr.slice(2, 4)),
    b: hexToDec(hexStr.slice(4, 6)),
    a: clampAlpha(a),
  };
}

function rgbToHex(r: number, g: number, b: number, a = 1, includeAlpha = a < 1): string {
  const hex = `#${decToHex(clampByte(r))}${decToHex(clampByte(g))}${decToHex(clampByte(b))}`;
  if (includeAlpha) {
    return `${hex}${decToHex(Math.round(clampAlpha(a) * 255))}`;
  }
  return hex;
}

function rgbToHsl(r: number, g: number, b: number, a = 1): { h: number; s: number; l: number; a: number } {
  let rr = clampByte(r) / 255;
  let gg = clampByte(g) / 255;
  let bb = clampByte(b) / 255;
  const max = Math.max(rr, gg, bb);
  const min = Math.min(rr, gg, bb);
  let h = 0;
  let s = 0;
  const l = (max + min) / 2;
  if (max !== min) {
    const delta = max - min;
    s = l > 0.5 ? delta / (2 - max - min) : delta / (max + min);
    switch (max) {
      case rr:
        h = (gg - bb) / delta + (gg < bb ? 6 : 0);
        break;
      case gg:
        h = (bb - rr) / delta + 2;
        break;
      default:
        h = (rr - gg) / delta + 4;
        break;
    }
    h *= 60;
  }
  return {
    h: Math.round(h),
    s: Math.round(s * 100),
    l: Math.round(l * 100),
    a: clampAlpha(a),
  };
}

function hslToRgb(h: number, s: number, l: number, a = 1): RgbaColor {
  const hue = ((h % 360) + 360) % 360;
  const saturation = Math.max(0, Math.min(100, s)) / 100;
  const lightness = Math.max(0, Math.min(100, l)) / 100;
  if (saturation === 0) {
    const channel = Math.round(lightness * 255);
    return { r: channel, g: channel, b: channel, a: clampAlpha(a) };
  }
  const hue2rgb = (p: number, q: number, t: number) => {
    let tt = t;
    if (tt < 0) tt += 1;
    if (tt > 1) tt -= 1;
    if (tt < 1 / 6) return p + (q - p) * 6 * tt;
    if (tt < 1 / 2) return q;
    if (tt < 2 / 3) return p + (q - p) * (2 / 3 - tt) * 6;
    return p;
  };
  const q = lightness < 0.5 ? lightness * (1 + saturation) : lightness + saturation - lightness * saturation;
  const p = 2 * lightness - q;
  return {
    r: Math.round(hue2rgb(p, q, hue / 360 + 1 / 3) * 255),
    g: Math.round(hue2rgb(p, q, hue / 360) * 255),
    b: Math.round(hue2rgb(p, q, hue / 360 - 1 / 3) * 255),
    a: clampAlpha(a),
  };
}

function parseRgbColor(color: string): RgbaColor | undefined {
  const match = color.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)(?:\s*,\s*([\d.]+))?\s*\)$/i);
  if (!match) return undefined;
  const r = parseRgbChannel(match[1]);
  const g = parseRgbChannel(match[2]);
  const b = parseRgbChannel(match[3]);
  const a = parseAlphaChannel(match[4]);
  if (r === undefined || g === undefined || b === undefined || a === undefined) return undefined;
  return { r, g, b, a: clampAlpha(a) };
}

function parseHslColor(color: string): RgbaColor | undefined {
  const match = color.match(/^hsla?\(\s*(\d+)\s*,\s*(\d+)%\s*,\s*(\d+)%(?:\s*,\s*([\d.]+))?\s*\)$/i);
  if (!match) return undefined;
  const h = Number.parseInt(match[1], 10);
  const s = parsePercentChannel(match[2]);
  const l = parsePercentChannel(match[3]);
  const a = parseAlphaChannel(match[4]);
  if (Number.isNaN(h) || s === undefined || l === undefined || a === undefined) return undefined;
  return hslToRgb(h, s, l, a);
}

export function parseCssColor(color: string): RgbaColor | undefined {
  const normalized = color.trim();
  if (!normalized || !exactCssColorPattern.test(normalized)) return undefined;
  if (normalized.startsWith('#')) {
    return hexToRgb(normalized);
  }
  if (/^rgb/i.test(normalized)) {
    return parseRgbColor(normalized);
  }
  if (/^hsl/i.test(normalized)) {
    return parseHslColor(normalized);
  }
  return undefined;
}

export function detectCssColorFormat(color: string): CssColorFormat | null {
  const normalized = color.trim().toLowerCase();
  if (!normalized) return null;
  if (normalized.startsWith('#')) {
    return normalized.length === 5 || normalized.length === 9 ? 'hexa' : 'hex';
  }
  if (normalized.startsWith('rgba(')) return 'rgba';
  if (normalized.startsWith('rgb(')) return 'rgb';
  if (normalized.startsWith('hsla(')) return 'hsla';
  if (normalized.startsWith('hsl(')) return 'hsl';
  return null;
}

function formatRgbColor(color: RgbaColor, includeAlpha = color.a < 1): string {
  const alpha = roundAlpha(color.a);
  return `rgb${includeAlpha ? 'a' : ''}(${clampByte(color.r)}, ${clampByte(color.g)}, ${clampByte(color.b)}${includeAlpha ? `, ${alpha}` : ''})`;
}

function formatHslColor(color: RgbaColor, includeAlpha = color.a < 1): string {
  const hsl = rgbToHsl(color.r, color.g, color.b, color.a);
  return `hsl${includeAlpha ? 'a' : ''}(${hsl.h}, ${hsl.s}%, ${hsl.l}%${includeAlpha ? `, ${roundAlpha(hsl.a)}` : ''})`;
}

export function formatCssColor(color: RgbaColor, format: CssColorFormat): string {
  switch (format) {
    case 'hexa':
      return rgbToHex(color.r, color.g, color.b, color.a, true);
    case 'hex':
      return rgbToHex(color.r, color.g, color.b, color.a, color.a < 1);
    case 'rgba':
      return formatRgbColor(color, true);
    case 'rgb':
      return formatRgbColor(color, color.a < 1);
    case 'hsla':
      return formatHslColor(color, true);
    case 'hsl':
      return formatHslColor(color, color.a < 1);
  }
}

function convertColor(color: string): { hex: string; rgb: string; hsl: string } | undefined {
  const rgba = parseCssColor(color);
  if (!rgba) return undefined;
  return {
    hex: rgbToHex(rgba.r, rgba.g, rgba.b, rgba.a, rgba.a < 1),
    rgb: formatRgbColor(rgba),
    hsl: formatHslColor(rgba),
  };
}

function isColor(value: string): boolean {
  return Boolean(parseCssColor(value));
}

export function toMonacoColor(color: RgbaColor): { red: number; green: number; blue: number; alpha: number } {
  return {
    red: clampByte(color.r) / 255,
    green: clampByte(color.g) / 255,
    blue: clampByte(color.b) / 255,
    alpha: clampAlpha(color.a),
  };
}

export function fromMonacoColor(color: { red: number; green: number; blue: number; alpha: number }): RgbaColor {
  return {
    r: Math.round(clampAlpha(color.red) * 255),
    g: Math.round(clampAlpha(color.green) * 255),
    b: Math.round(clampAlpha(color.blue) * 255),
    a: clampAlpha(color.alpha),
  };
}

export function getCssColorMatches(text: string): Array<{ text: string; start: number; end: number }> {
  const matches = text.matchAll(new RegExp(cssColorTokenSource, 'gi'));
  return Array.from(matches, (match) => ({
    text: match[0],
    start: match.index ?? 0,
    end: (match.index ?? 0) + match[0].length,
  }));
}

export const colorPreviewer: Previewer = {
  detector: ({ value }) => isColor(value),
  generator: ({ value }) => {
    const converted = convertColor(value);
    if (!converted) return '';
    const swatch = `<div style="width:128px;height:16px;background-color:${converted.hex};border:1px solid #cbd5e1;border-radius:4px;"></div>`;
    const table = buildTable({
      HEX: converted.hex,
      RGB: converted.rgb,
      HSL: converted.hsl,
    });
    return joinSections([swatch, table]);
  },
};
