import type { Previewer } from './types';
import { joinSections, wrapHeading, wrapPre } from './utils';

function decodeBase64Url(value: string): string {
  let base64 = value.replace(/-/g, '+').replace(/_/g, '/');
  const padLength = 4 - (base64.length % 4);
  if (padLength < 4) {
    base64 += '='.repeat(padLength);
  }
  return atob(base64);
}

export const jwtPreviewer: Previewer = {
  detector: ({ value }) => /^(eyJ[A-Za-z0-9_/-]+\.){2}([A-Za-z0-9_/-]+)?$/.test(value),
  generator: ({ value }) => {
    try {
      const [headerPart, payloadPart, signaturePart = ''] = value.split('.');
      const header = JSON.parse(decodeBase64Url(headerPart));
      const payload = JSON.parse(decodeBase64Url(payloadPart));
      return joinSections([
        wrapHeading('JWT Header'),
        wrapPre(JSON.stringify(header, null, 2)),
        wrapHeading('JWT Payload'),
        wrapPre(JSON.stringify(payload, null, 2)),
        wrapHeading(`Signature Length: ${signaturePart.length}`),
      ]);
    } catch {
      return '';
    }
  },
};
