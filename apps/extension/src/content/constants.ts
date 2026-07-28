// Content scripts are classic Manifest V3 scripts, not ES modules. Keep their
// tiny, page-interaction-only limits in this entry-local module so Vite emits a
// self-contained content.js instead of a shared ESM import chunk.
export const MAX_CANDIDATE_BYTES = 1024 * 1024;
export const MAX_ANCESTOR_LEVELS = 3;
export const CLICK_DEDUP_WINDOW_MS = 500;
