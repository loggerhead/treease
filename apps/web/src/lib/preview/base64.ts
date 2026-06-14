import type { Previewer } from './types';
import { joinSections, wrapHeading, wrapPre } from './utils';

export const base64Previewer: Previewer = {
  detector: ({ value }) => {
    if (value.length === 0) return false;
    if (/^\d+(\.\d+)?$/.test(value)) return false;
    const normalized = value.replace(/-/g, '+').replace(/_/g, '/');
    if (!/^[A-Za-z0-9+/]*=?=?$/.test(normalized)) return false;
    if (normalized.length % 4 !== 0) return false;
    const upperProb = (value.match(/[A-Z]/g) || []).length / value.length;
    const lowerProb = (value.match(/[a-z]/g) || []).length / value.length;
    const numberProb = (value.match(/[0-9]/g) || []).length / value.length;
    if (upperProb < 0.1 && lowerProb < 0.1 && numberProb < 0.04) return false;
    try {
      atob(normalized);
      return true;
    } catch {
      return false;
    }
  },
  generator: ({ value }) => {
    const normalized = value.replace(/-/g, '+').replace(/_/g, '/');
    const decoded = atob(normalized);
    return joinSections([wrapHeading('Base64 Decoded'), wrapPre(decoded)]);
  },
};
