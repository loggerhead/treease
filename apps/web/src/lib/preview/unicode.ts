import type { Previewer } from './types';
import { wrapPre } from './utils';

export const unicodePreviewer: Previewer = {
  detector: ({ rawValue }) => /\\u[0-9a-fA-F]{4}/.test(rawValue),
  generator: ({ value }) => wrapPre(value),
};
