import { describe, expect, it } from 'vitest';

import { canExecuteUrlCommandForLanguage, resolveEditorUrlPreset, summarizeEditorUrlPresetWarnings } from './url-preset';

describe('editor url preset', () => {
  it('parses whitelist ui tokens case-insensitively and deduplicates them', () => {
    const preset = resolveEditorUrlPreset('?ui=Viewer,editor,VIEWER,topbar');

    expect(preset.telemetry.recognized.ui).toEqual(['editor', 'viewer', 'topbar']);
    expect(preset.ui).toEqual({
      editor: true,
      viewer: true,
      topbar: true,
      bottombar: false,
    });
  });

  it('lets yq take precedence over rightText and force viewer text mode', () => {
    const preset = resolveEditorUrlPreset('?ui=editor&rightText=%7B%7D&yq=to_yaml');

    expect(preset.rightText.effective).toBe(false);
    expect(preset.yq.effective).toBe(true);
    expect(preset.ui.viewer).toBe(true);
    expect(preset.initialViewerMode).toBe('text');
    expect(preset.notes).toContain('Ignored rightText because yq takes precedence.');
  });

  it('lets a valid command suppress yq but not an invalid command', () => {
    const commandPreset = resolveEditorUrlPreset('?command=compare&yq=to_yaml');
    expect(commandPreset.command).toBe('compare');
    expect(commandPreset.yq.effective).toBe(false);
    expect(commandPreset.notes).toContain('Ignored yq because command=compare takes precedence.');

    const invalidPreset = resolveEditorUrlPreset('?command=unknown&yq=to_yaml');
    expect(invalidPreset.command).toBeNull();
    expect(invalidPreset.yq.effective).toBe(true);
    expect(invalidPreset.telemetry.ignored).toContain('command=unknown');
  });

  it('keeps explicit empty text and rightText as present clears', () => {
    const preset = resolveEditorUrlPreset('?text=&rightText=');

    expect(preset.text).toEqual({ present: true, value: '' });
    expect(preset.rightText).toEqual({ present: true, value: '', effective: true });
    expect(preset.initialViewerMode).toBe('text');
  });

  it('lets inline text and rightText take precedence over url-backed variants', () => {
    const preset = resolveEditorUrlPreset(
      '?text=%7B%22inline%22%3Atrue%7D&textUrl=%2Ffixtures%2Fleft.json&rightText=%7B%22right%22%3A1%7D&rightTextUrl=%2Ffixtures%2Fright.json',
    );

    expect(preset.textUrl.effective).toBe(false);
    expect(preset.rightTextUrl.effective).toBe(false);
    expect(preset.notes).toContain('Ignored textUrl because text takes precedence.');
    expect(preset.notes).toContain('Ignored rightTextUrl because rightText takes precedence.');
  });

  it('lets yq suppress rightTextUrl and still force viewer text mode', () => {
    const preset = resolveEditorUrlPreset('?rightTextUrl=%2Ffixtures%2Fright.json&yq=to_yaml');

    expect(preset.rightTextUrl.effective).toBe(false);
    expect(preset.yq.effective).toBe(true);
    expect(preset.ui.viewer).toBe(true);
    expect(preset.initialViewerMode).toBe('text');
    expect(preset.notes).toContain('Ignored rightTextUrl because yq takes precedence.');
  });

  it('summarizes warnings from ignored values and precedence notes', () => {
    const preset = resolveEditorUrlPreset('?ui=editor,unknown&command=compare&yq=to_yaml');

    expect(summarizeEditorUrlPresetWarnings(preset)).toBe(
      'Editor URL preset warnings: ui=unknown Ignored yq because command=compare takes precedence.',
    );
  });

  it('enforces url command language restrictions', () => {
    expect(canExecuteUrlCommandForLanguage('escape', 'json')).toBe(true);
    expect(canExecuteUrlCommandForLanguage('escape', 'yaml')).toBe(false);
    expect(canExecuteUrlCommandForLanguage('format', 'yaml')).toBe(true);
  });
});
