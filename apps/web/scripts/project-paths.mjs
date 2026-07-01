import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

function isWorkspaceRoot(dir) {
  return (
    existsSync(join(dir, 'package.json')) &&
    existsSync(join(dir, 'README.md')) &&
    existsSync(join(dir, 'apps/web/package.json'))
  );
}

export function workspaceRootFromScript(scriptUrl) {
  let current = dirname(fileURLToPath(scriptUrl));

  while (!isWorkspaceRoot(current)) {
    const parent = dirname(current);
    if (parent === current) {
      throw new Error(`Failed to locate Treease workspace root from ${scriptUrl}`);
    }
    current = parent;
  }

  return current;
}

export function fromScriptsDir(scriptUrl, relativePath) {
  const workspaceRoot = workspaceRootFromScript(scriptUrl);
  return resolve(workspaceRoot, 'apps/web/scripts', relativePath);
}
