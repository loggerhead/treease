import { base64Previewer } from './base64';
import { colorPreviewer } from './color';
import { datePreviewer } from './date';
import { imagePreviewer } from './image';
import { jwtPreviewer } from './jwt';
import type { PreviewContext, Previewer } from './types';
import { unicodePreviewer } from './unicode';
import { uriPreviewer } from './uri';
import { urlPreviewer } from './url';

const previewers: Previewer[] = [
  imagePreviewer,
  urlPreviewer,
  datePreviewer,
  colorPreviewer,
  base64Previewer,
  uriPreviewer,
  jwtPreviewer,
  unicodePreviewer,
];

export async function generatePreview(context: PreviewContext) {
  for (const previewer of previewers) {
    if (await previewer.detector(context)) {
      return previewer.generator(context);
    }
  }
  return null;
}
