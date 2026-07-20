import { describe, expect, it } from 'vitest';
import config from '../svelte.config.js';

describe('SvelteKit asset paths', () => {
  it('emits root-relative asset URLs for the Workers deployment', () => {
    expect(config.kit.paths?.relative).toBe(false);
  });
});
