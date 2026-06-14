import type { ClassValue } from 'svelte/elements';

export type { ClassValue };

function toClassArray(value: ClassValue): string[] {
  if (typeof value === 'string') return value.trim().split(/\s+/).filter(Boolean);
  if (Array.isArray(value)) return value.flatMap(toClassArray);
  if (typeof value === 'object' && value !== null) {
    return Object.entries(value)
      .filter(([, v]) => v)
      .map(([k]) => k);
  }
  return [];
}

export function cn(...values: ClassValue[]) {
  return values.flatMap(toClassArray).join(' ');
}

export type WithoutChildrenOrChild<T> = Omit<T, 'children' | 'child'>;
export type WithoutChild<T> = Omit<T, 'child'>;
export type WithElementRef<T> = T & { ref?: HTMLElement | null };
