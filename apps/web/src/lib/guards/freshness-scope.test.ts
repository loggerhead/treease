import { describe, expect, it } from 'vitest';
import { createFreshnessScope } from './freshness-scope';

describe('freshness-scope', () => {
  it('accepts an unchanged document context', () => {
    let revision = 3;
    const scope = createFreshnessScope(
      { documentKey: 'doc-key', revision },
      () => ({ documentKey: 'doc-key', revision }),
    );

    expect(scope.isCurrent()).toBe(true);
  });

  it('rejects stale document revisions', () => {
    let revision = 3;
    const scope = createFreshnessScope(
      { documentKey: 'doc-key', revision },
      () => ({ documentKey: 'doc-key', revision }),
    );
    revision = 4;

    expect(scope.isCurrent()).toBe(false);
  });

  it('rejects stale model versions', () => {
    let version = 3;
    const model = {
      getVersionId: () => version,
    };
    const scope = createFreshnessScope(
      { documentKey: 'doc-key', languageId: 'json', model },
      () => ({ documentKey: 'doc-key', languageId: 'json', model }),
    );
    version = 4;

    expect(scope.isCurrent()).toBe(false);
  });

  it('drops async step results after the token changes', async () => {
    let token = 1;
    const scope = createFreshnessScope(
      { token },
      () => ({ token }),
    );

    const result = await scope.step(async () => {
      token = 2;
      return 'stale-result';
    });

    expect(result).toBeNull();
  });
});
