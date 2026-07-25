import { describe, it, expect } from 'vitest';
import { commandItems } from './command-registry';

describe('command-registry', () => {
  it('exports unique command ids with searchable metadata', () => {
    const ids = commandItems.map((c) => c.id);
    expect(ids).toContain('format');
    expect(ids).toContain('minify');
    expect(ids).toContain('compact');
    expect(ids).toContain('sort');
    expect(ids).toContain('show-yq-input');
    expect(ids).toContain('generate-struct');
    expect(ids).toContain('escape');
    expect(ids).toContain('unescape');
    expect(ids).not.toEqual(expect.arrayContaining([
      'workspace:new',
      'workspace:open',
      'workspace:save',
      'workspace:save-as',
      'workspace:close-tab',
    ]));
    expect(new Set(ids).size).toBe(ids.length);
    for (const cmd of commandItems) {
      expect(cmd.label).toBeTruthy();
      expect(cmd.keywords).toBeInstanceOf(Array);
      expect(cmd.keywords.length).toBeGreaterThan(0);
      expect(cmd.langs).toBeInstanceOf(Array);
      expect(cmd.langs.length).toBeGreaterThan(0);
    }
  });

  it('marks universal commands with langs: ["*"]', () => {
    const universal = commandItems.filter((c) => c.langs.includes('*'));
    const ids = universal.map((c) => c.id);
    expect(ids).toEqual(
      expect.arrayContaining(['format', 'minify', 'compact', 'sort', 'show-yq-input', 'toggle-auto-format']),
    );
  });

  it('restricts JSON structure generation to json', () => {
    const generateStruct = commandItems.find((c) => c.id === 'generate-struct')!;
    expect(generateStruct.langs).toEqual(['json']);
  });

  it('restricts toggle-nest/escape/unescape to json', () => {
    const toggleNest = commandItems.find((c) => c.id === 'toggle-nest')!;
    expect(toggleNest.langs).toEqual(['json']);
    const escape = commandItems.find((c) => c.id === 'escape')!;
    expect(escape.langs).toEqual(['json']);
    const unescape = commandItems.find((c) => c.id === 'unescape')!;
    expect(unescape.langs).toEqual(['json']);
  });
  it('filters commands by language', () => {
    const jsonCommands = commandItems.filter((c) => c.langs.includes('*') || c.langs.includes('json'));
    const yamlCommands = commandItems.filter((c) => c.langs.includes('*') || c.langs.includes('yaml'));
    expect(jsonCommands.find((c) => c.id === 'escape')).toBeTruthy();
    expect(yamlCommands.find((c) => c.id === 'escape')).toBeFalsy();
    expect(yamlCommands.find((c) => c.id === 'generate-struct')).toBeFalsy();
  });
});
