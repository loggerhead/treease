import type { Previewer } from './types';
import { joinSections, wrapHeading, wrapPre } from './utils';

export const uriPreviewer: Previewer = {
  detector: ({ value }) => {
    try {
      return decodeURIComponent(value) !== value;
    } catch {
      return false;
    }
  },
  generator: ({ value }) => {
    const decoded = decodeURIComponent(value);
    return joinSections([wrapHeading('URI Decoded'), wrapPre(decoded)]);
  },
};
