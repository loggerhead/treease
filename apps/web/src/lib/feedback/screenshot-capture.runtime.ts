import { toPng } from 'html-to-image';

export function captureFeedbackScreenshot(
  target: HTMLElement,
  filter: (node: HTMLElement) => boolean,
): Promise<string> {
  return toPng(target, {
    cacheBust: true,
    skipFonts: true,
    pixelRatio: 1,
    filter,
  });
}
