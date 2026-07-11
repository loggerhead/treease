import path from 'node:path';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import ts from 'typescript';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..', '..', '..');
const require = createRequire(import.meta.url);
const knownSourceExtensions = new Set(['.ts', '.tsx', '.js', '.mjs', '.cjs', '.json']);

function hasExtension(specifier) {
  return knownSourceExtensions.has(path.extname(specifier));
}

function resolveFile(target) {
  if (fs.existsSync(target)) return target;
  if (!hasExtension(target)) {
    const ts = `${target}.ts`;
    if (fs.existsSync(ts)) return ts;
    const js = `${target}.js`;
    if (fs.existsSync(js)) return js;
  }
  return target;
}

export async function resolve(specifier, context, nextResolve) {
  if (specifier === '@core-wasm/pkg') {
    const target = path.join(repoRoot, 'packages', 'core', 'wasm', 'pkg', 'core.js');
    return nextResolve(pathToFileURL(target).href, context);
  }

  if (specifier.startsWith('@core-wasm/')) {
    const subPath = specifier.slice('@core-wasm/'.length);
    const target = resolveFile(path.join(repoRoot, 'packages', 'core', 'wasm', subPath));
    const url = pathToFileURL(target).href;
    return nextResolve(url, context);
  }

  if (specifier.startsWith('@core/')) {
    const subPath = specifier.slice('@core/'.length);
    const target = resolveFile(path.join(repoRoot, 'packages', 'core', 'src', subPath));
    const url = pathToFileURL(target).href;
    return nextResolve(url, context);
  }

  if (
    (specifier.startsWith('./') || specifier.startsWith('../') || specifier.startsWith('/')) &&
    !hasExtension(specifier)
  ) {
    try {
      return await nextResolve(`${specifier}.ts`, context);
    } catch {
      return nextResolve(specifier, context);
    }
  }

  if (
    !specifier.startsWith('node:') &&
    !specifier.startsWith('file:') &&
    !specifier.startsWith('./') &&
    !specifier.startsWith('../') &&
    !specifier.startsWith('/')
  ) {
    try {
      const resolvedPath = require.resolve(specifier);
      const url = pathToFileURL(resolvedPath).href;
      return nextResolve(url, context);
    } catch {
      // fall through
    }
  }

  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (url.startsWith('file:') && (url.endsWith('.ts') || url.endsWith('.tsx'))) {
    const filename = fileURLToPath(url);
    const source = fs.readFileSync(filename, 'utf8');
    const result = ts.transpileModule(source, {
      fileName: filename,
      compilerOptions: {
        module: ts.ModuleKind.ESNext,
        target: ts.ScriptTarget.ES2022,
        importHelpers: false,
        sourceMap: false,
        inlineSourceMap: false,
        inlineSources: false,
      },
    });
    return { format: 'module', source: result.outputText, shortCircuit: true };
  }
  return nextLoad(url, context);
}
