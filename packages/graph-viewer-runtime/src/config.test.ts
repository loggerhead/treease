import { describe, expect, it } from 'vitest';
import { defaultGraphViewerRenderConfig } from './config';
import { semanticTypeToColorKey } from './semantic-type-color';

describe('graph-viewer runtime defaults', () => {
  it('keeps the Core semantic type mapping independent from Web', () => {
    expect(semanticTypeToColorKey(0)).toBe('map');
    expect(semanticTypeToColorKey(6)).toBe('nil');
    expect(defaultGraphViewerRenderConfig.layout.canvasPadding).toBe(50);
  });
});
