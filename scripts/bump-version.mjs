import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = parseArgs();
const bumpPart = args.part;

const TARGETS = {
  core: {
    tag: 'v[0-9]*',
    paths: ['packages/core/'],
    version: () => readTomlStringValue(readFile('packages/core/Cargo.toml'), 'package', 'version'),
  },
  web: {
    tag: 'v[0-9]*',
    paths: ['apps/web/', 'packages/core/', 'packages/api-contracts/', 'packages/share-protocol/', 'packages/graph-viewer-runtime/'],
    version: () => readPackageVersion('apps/web/package.json'),
  },
  cli: {
    tag: 'cli-v[0-9]*',
    paths: ['apps/cli/', 'packages/core/'],
    version: () => readTomlStringValue(readFile('apps/cli/Cargo.toml'), 'package', 'version'),
  },
  desktop: {
    tag: 'desktop-v[0-9]*',
    paths: ['apps/desktop/', 'apps/web/', 'packages/core/', 'packages/api-contracts/', 'packages/share-protocol/', 'packages/graph-viewer-runtime/'],
    version: () => readPackageVersion('apps/desktop/package.json'),
  },
  extension: {
    tag: 'extension-v[0-9]*',
    paths: ['apps/extension/', 'packages/graph-viewer-runtime/'],
    version: () => readPackageVersion('apps/extension/package.json'),
  },
};

const targetTags = new Map();
const targets = args.targets ?? resolveTargets();
if (targets.size === 0) {
  fail('No release artifact changed since its latest tag.');
}

const currentVersions = Object.fromEntries([...targets].map((target) => [target, TARGETS[target].version()]));
const nextVersions = Object.fromEntries(
  [...targets].map((target) => [target, nextVersion(target, currentVersions[target])])
);

const updates = buildUpdates(targets, nextVersions);
if (args.check) {
  for (const target of targets) {
    console.log(`would bump ${target}: ${currentVersions[target]} -> ${nextVersions[target]}`);
  }
  for (const update of updates) {
    console.log(`would update ${update.file}`);
  }
  process.exit(0);
}

for (const update of updates) {
  writeFileSync(path.resolve(rootDir, update.file), update.contents);
}
for (const target of targets) {
  console.log(`bumped ${target}: ${currentVersions[target]} -> ${nextVersions[target]}`);
}

function resolveTargets() {
  const resolved = new Set();
  for (const [target, definition] of Object.entries(TARGETS)) {
    const tag = latestTag(definition.tag);
    if (!tag) {
      console.warn(`${target}: no release tag matches ${definition.tag}; skipping un-released artifact`);
      continue;
    }
    const changedFiles = git('diff', '--name-only', tag, '--').split('\n').filter(Boolean);
    if (changedFiles.some((file) => definition.paths.some((prefix) => file.startsWith(prefix)))) {
      resolved.add(target);
      targetTags.set(target, tag);
      console.log(`${target}: ${tag}..working tree changed`);
    }
  }
  return resolved;
}

function nextVersion(target, currentVersion) {
  const tag = targetTags.get(target) ?? latestTag(TARGETS[target].tag);
  const tagVersion = tag?.match(/(\d+\.\d+\.\d+)$/)?.[1];
  if (!tagVersion) return bumpSemver(currentVersion, bumpPart);
  const expected = bumpSemver(tagVersion, bumpPart);
  return currentVersion === expected ? currentVersion : bumpSemver(currentVersion, bumpPart);
}

function buildUpdates(targets, nextVersions) {
  const updates = [];
  if (targets.has('core')) {
    const corePath = 'packages/core/Cargo.toml';
    const coreContent = updateTomlField(readFile(corePath), 'package', 'version', nextVersions.core);
    updates.push({ file: corePath, contents: coreContent });

    const wasmPath = 'packages/core/wasm/pkg/package.json';
    if (existsSync(path.resolve(rootDir, wasmPath))) {
      const pkg = JSON.parse(readFile(wasmPath));
      pkg.version = nextVersions.core;
      pkg.name = readTomlStringValue(coreContent, 'package', 'name');
      updates.push({ file: wasmPath, contents: `${JSON.stringify(pkg, null, 2)}\n` });
    }
  }

  if (targets.has('web')) {
    updates.push({ file: 'apps/web/package.json', contents: updateJsonField(readFile('apps/web/package.json'), 'version', nextVersions.web) });
  }

  if (targets.has('cli')) {
    const cliPath = 'apps/cli/Cargo.toml';
    let cliContent = readFile(cliPath);
    cliContent = updateTomlField(cliContent, 'package', 'version', nextVersions.cli);
    const coreVersion = nextVersions.core ?? readTomlStringValue(readFile('packages/core/Cargo.toml'), 'package', 'version');
    cliContent = updateTomlDependencyVersion(cliContent, 'dependencies', 'treease-core', coreVersion);
    updates.push({ file: cliPath, contents: cliContent });
  }

  if (targets.has('desktop')) {
    updates.push({ file: 'apps/desktop/package.json', contents: updateJsonField(readFile('apps/desktop/package.json'), 'version', nextVersions.desktop) });
    updates.push({ file: 'apps/desktop/src-tauri/Cargo.toml', contents: updateTomlField(readFile('apps/desktop/src-tauri/Cargo.toml'), 'package', 'version', nextVersions.desktop) });
    updates.push({ file: 'apps/desktop/src-tauri/tauri.conf.json', contents: updateJsonField(readFile('apps/desktop/src-tauri/tauri.conf.json'), 'version', nextVersions.desktop) });
  }

  if (targets.has('extension')) {
    updates.push({ file: 'apps/extension/package.json', contents: updateJsonField(readFile('apps/extension/package.json'), 'version', nextVersions.extension) });
    updates.push({ file: 'apps/extension/public/manifest.json', contents: updateJsonField(readFile('apps/extension/public/manifest.json'), 'version', nextVersions.extension) });
  }

  const rustVersions = new Map([
    ['core', ['treease-core', nextVersions.core]],
    ['cli', ['treease-cli', nextVersions.cli]],
    ['desktop', ['treease-desktop', nextVersions.desktop]],
  ]);
  let lockContents = null;
  for (const [target, [packageName, version]] of rustVersions) {
    if (!targets.has(target)) continue;
    lockContents ??= readFile('Cargo.lock');
    lockContents = updateLockPackageVersion(lockContents, packageName, version);
  }
  if (lockContents !== null) updates.push({ file: 'Cargo.lock', contents: lockContents });
  return updates;
}

function latestTag(pattern) {
  const tags = git('tag', '--list', pattern, '--sort=-version:refname').split('\n').filter(Boolean);
  return tags[0] ?? null;
}

function git(...command) {
  return execFileSync('git', command, { cwd: rootDir, encoding: 'utf8' }).trim();
}

function parseArgs() {
  const parsed = { check: false, targets: null, part: 'patch' };
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--check') parsed.check = true;
    else if (arg === '--part') parsed.part = argv[++i];
    else if (arg.startsWith('--part=')) parsed.part = arg.slice('--part='.length);
    else if (arg === '--targets') parsed.targets = parseTargetList(argv[++i]);
    else if (arg.startsWith('--targets=')) parsed.targets = parseTargetList(arg.slice('--targets='.length));
    else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else fail(`unknown argument ${arg}`);
  }
  if (!['patch', 'minor', 'major'].includes(parsed.part)) fail('only --part patch|minor|major are supported');
  return parsed;
}

function parseTargetList(value) {
  if (!value) fail('missing value for --targets');
  const targets = new Set(value.split(',').map((item) => item.trim()).filter(Boolean));
  for (const target of targets) {
    if (!TARGETS[target]) fail(`unknown target ${target}, expected ${Object.keys(TARGETS).join(',')}`);
  }
  return targets;
}

function readFile(relativePath) {
  return readFileSync(path.resolve(rootDir, relativePath), 'utf8');
}

function readPackageVersion(relativePath) {
  const version = JSON.parse(readFile(relativePath)).version;
  if (!/^\d+\.\d+\.\d+$/.test(version)) fail(`invalid version ${version} in ${relativePath}`);
  return version;
}

function bumpSemver(value, part) {
  const match = value.match(/^(\d+)\.(\d+)\.(\d+)$/);
  if (!match) fail(`invalid semver: ${value}`);
  let [major, minor, patch] = match.slice(1).map(Number);
  if (part === 'major') [major, minor, patch] = [major + 1, 0, 0];
  else if (part === 'minor') [major, minor, patch] = [major, minor + 1, 0];
  else patch += 1;
  return `${major}.${minor}.${patch}`;
}

function updateTomlField(contents, sectionName, key, nextValue) {
  const lines = contents.split(/\r?\n/);
  let section = null;
  let updated = false;
  const next = lines.map((line) => {
    const sectionMatch = line.match(/^\[([^\]]+)\]/);
    if (sectionMatch) section = sectionMatch[1];
    if (section === sectionName) {
      const match = line.match(new RegExp(`^(\\s*)${escapeRegExp(key)}\\s*=\\s*"`));
      if (match) {
        updated = true;
        return `${match[1]}${key} = "${nextValue}"`;
      }
    }
    return line;
  });
  if (!updated) fail(`failed to update ${sectionName}.${key}`);
  return `${next.join('\n').replace(/\n+$/, '')}\n`;
}

function updateTomlDependencyVersion(contents, sectionName, dependencyName, nextVersion) {
  const lines = contents.split(/\r?\n/);
  let section = null;
  let updated = false;
  const dependency = escapeRegExp(dependencyName);
  const entry = new RegExp(`^(\\s*)${dependency}\\s*=\\s*\\{([^}]*)\\}$`);
  const version = new RegExp(`(\\bversion\\s*=\\s*")([^"]+)(")`);
  const next = lines.map((line) => {
    const sectionMatch = line.match(/^\[([^\]]+)\]/);
    if (sectionMatch) section = sectionMatch[1];
    if (section === sectionName && entry.test(line)) {
      if (!version.test(line)) fail(`unable to parse dependency entry for ${dependencyName}`);
      updated = true;
      return line.replace(version, `$1${nextVersion}$3`);
    }
    return line;
  });
  if (!updated) fail(`failed to update dependency ${dependencyName} in [${sectionName}]`);
  return `${next.join('\n').replace(/\n+$/, '')}\n`;
}

function updateJsonField(contents, key, nextValue) {
  const regex = new RegExp(`("${escapeRegExp(key)}"\\s*:\\s*")([^"]+)(")`);
  if (!regex.test(contents)) fail(`failed to update ${key}`);
  return contents.replace(regex, `$1${nextValue}$3`);
}

function updateLockPackageVersion(contents, packageName, nextVersion) {
  const packagePattern = new RegExp(`(name = "${escapeRegExp(packageName)}"\\nversion = ")([^"]+)(")`);
  if (!packagePattern.test(contents)) fail(`missing ${packageName} in Cargo.lock`);
  return contents.replace(packagePattern, `$1${nextVersion}$3`);
}

function readTomlStringValue(contents, sectionName, key) {
  const section = getTomlSection(contents, sectionName);
  const match = section.match(new RegExp(`^\\s*${escapeRegExp(key)}\\s*=\\s*"([^"]+)"`, 'm'));
  if (!match) fail(`missing ${sectionName}.${key}`);
  return match[1];
}

function getTomlSection(contents, sectionName) {
  const lines = contents.split(/\r?\n/);
  let current = null;
  const sectionLines = [];
  for (const line of lines) {
    const sectionMatch = line.match(/^\[([^\]]+)\]/);
    if (sectionMatch) current = sectionMatch[1];
    else if (current === sectionName) sectionLines.push(line);
  }
  if (sectionLines.length === 0) fail(`missing section [${sectionName}]`);
  return sectionLines.join('\n');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function printHelp() {
  console.log('Usage: node scripts/bump-version.mjs [--check] [--part patch|minor|major] [--targets target,...]');
  console.log('Without --targets, changed artifacts are inferred from each artifact\'s latest release tag.');
  console.log(`Targets: ${Object.keys(TARGETS).join(', ')}`);
}

function fail(message) {
  console.error(`[bump-version] ${message}`);
  process.exit(1);
}
