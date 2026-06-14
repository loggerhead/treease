import type { Previewer } from './types';
import { escapeHtml } from './utils';

export const imagePreviewer: Previewer = {
  detector: ({ value }) => {
    if (/^https?:\/\/.*/.test(value)) {
      return /\.(png|jpg|jpeg|gif|webp|bmp|svg)(?:$|[?#])/i.test(value);
    }
    return /^data:image\/\w+;base64,/i.test(value);
  },
  generator: ({ value }) => `<img src="${escapeHtml(value)}">`,
};
