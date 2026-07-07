import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(here, '..');

const targets = parseTargets();
const bumpPart = parsePart();

if (targets.size === 0) {
  fail('No target specified. Use --targets core,cli,web');
}

const cliManifestPath = path.resolve(rootDir, 'apps', 'cli', 'Cargo.toml');
const coreManifestPath = path.resolve(rootDir, 'packages', 'core', 'Cargo.toml');
const webManifestPath = path.resolve(rootDir, 'apps', 'web', 'package.json');
const currentVersions = {
  web: readPackageVersion(webManifestPath),
};
let coreContent = null;
let cliContent = null;
let coreVersion = null;

if (targets.has('core') || targets.has('cli')) {
  coreContent = readFileSync(coreManifestPath, 'utf8');
  coreVersion = readTomlStringValue(coreContent, 'package', 'version');
  currentVersions.core = coreVersion;
}

if (targets.has('cli')) {
  cliContent = readFileSync(cliManifestPath, 'utf8');
  currentVersions.cli = readTomlStringValue(cliContent, 'package', 'version');
  if (targets.has('core')) {
    const currentCliDep = readInlineDependencyVersion(cliContent, 'treease-core');
    if (currentCliDep !== coreVersion) {
      fail(`apps/cli/Cargo.toml currently depends on treease-core ${currentCliDep}, expected ${coreVersion}`);
    }
  }
}

const nextVersions = {};
for (const target of targets) {
  nextVersions[target] = bumpSemver(currentVersions[target], bumpPart);
}

if (targets.has('core')) {
  const coreManifest = coreManifestPath;
  const updatedCore = updateTomlVersion(coreContent, 'package', 'version', nextVersions.core);
  writeFileSync(coreManifest, updatedCore);

  const coreWasmPackageJson = path.resolve(rootDir, 'packages', 'core', 'wasm', 'pkg', 'package.json');
  if (existsSync(coreWasmPackageJson)) {
    const coreWasmPackage = readFileSync(coreWasmPackageJson, 'utf8');
    const normalizedName = readTomlStringValue(coreContent, 'package', 'name');
    const pkg = JSON.parse(coreWasmPackage);
    pkg.version = nextVersions.core;
    if (pkg.name !== normalizedName) {
      pkg.name = normalizedName;
    }
    writeFileSync(coreWasmPackageJson, `${JSON.stringify(pkg, null, 2)}\n`);
  } else {
    console.warn(`Skip missing generated file: packages/core/wasm/pkg/package.json`);
  }
}

if (targets.has('cli')) {
  const cliManifest = cliManifestPath;
  const nextCliCoreVersion = targets.has('core') ? nextVersions.core : coreVersion;
  const withCliVersion = updateTomlVersion(cliContent, 'package', 'version', nextVersions.cli);
  const withDependency = updateTomlDependencyVersion(withCliVersion, 'dependencies', 'treease-core', nextCliCoreVersion);
  writeFileSync(cliManifest, withDependency);
}

if (targets.has('web')) {
  const updatedWeb = updateJsonField(
    readFileSync(webManifestPath, 'utf8'),
    'version',
    nextVersions.web
  );
  writeFileSync(webManifestPath, updatedWeb);
}

if (targets.has('cli') && targets.has('core')) {
  if (nextVersions.core !== nextVersions.cli) {
    console.log(`bumped core to ${nextVersions.core}, cli to ${nextVersions.cli}`);
    console.log(`set apps/cli/Cargo.toml treease-core dependency to ${nextVersions.core}`);
  } else {
    console.log(`bumped core and cli to ${nextVersions.core}`);
  }
}

for (const target of targets) {
  const targetVersion = nextVersions[target];
  console.log(`bumped ${target}: ${currentVersions[target]} -> ${targetVersion}`);
}

function bumpSemver(value, part) {
  const match = value.match(/^(\d+)\.(\d+)\.(\d+)$/);
  if (!match) {
    fail(`invalid semver: ${value}`);
  }
  let [major, minor, patch] = match.slice(1).map(Number);
  if (part === 'major') {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (part === 'minor') {
    minor += 1;
    patch = 0;
  } else {
    patch += 1;
  }
  return `${major}.${minor}.${patch}`;
}

function parseTargets() {
  const targets = new Set();
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--') {
      continue;
    }
    if (arg === '--targets') {
      const value = args[i + 1];
      if (!value) fail('missing value for --targets');
      appendTargets(value, targets);
      i += 1;
      continue;
    }
    if (arg.startsWith('--targets=')) {
      appendTargets(arg.slice('--targets='.length), targets);
      continue;
    }
    if (arg === '--target') {
      const value = args[i + 1];
      if (!value) fail('missing value for --target');
      appendTargets(value, targets);
      i += 1;
      continue;
    }
    if (arg === '--part') {
      i += 1;
      continue;
    }
    if (arg.startsWith('--part=')) {
      continue;
    }
    if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    }
    fail(`Unknown argument: ${arg}`);
  }
  return targets;
}

function appendTargets(value, targets) {
  const allowed = new Set(['core', 'cli', 'web']);
  for (const item of value.split(',')) {
    const target = item.trim();
    if (!target) continue;
    if (!allowed.has(target)) {
      fail(`unknown target ${target}, expected one of core,cli,web`);
    }
    targets.add(target);
  }
}

function parsePart() {
  const args = process.argv.slice(2);
  let part = 'patch';
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--') {
      continue;
    }
    if (arg === '--part') {
      const value = args[i + 1];
      if (!value) fail('missing value for --part');
      part = value;
      break;
    }
    if (arg.startsWith('--part=')) {
      part = arg.slice('--part='.length);
      break;
    }
  }
  if (!['patch', 'minor', 'major'].includes(part)) {
    fail('only --part patch|minor|major are supported');
  }
  return part;
}

function readPackageVersion(manifestPath) {
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const version = manifest.version;
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    fail(`invalid web package version ${version} in ${path.relative(rootDir, manifestPath)}`);
  }
  return version;
}

function readInlineDependencyVersion(contents, dependencyName) {
  const dependencies = getTomlSection(contents, 'dependencies');
  const dependencyPattern = new RegExp(
    `^\\s*${escapeRegExp(dependencyName)}\\s*=\\s*\\{[^\\n}]*\\bversion\\s*=\\s*"([^"]+)"[^\\n}]*\\}\\s*$`,
    'm'
  );
  const match = dependencies.match(dependencyPattern);
  if (!match) {
    fail(`missing ${dependencyName} inline dependency version`);
  }
  return match[1];
}

function updateTomlVersion(contents, sectionName, key, nextVersion) {
  return updateTomlField(contents, sectionName, key, nextVersion);
}

function updateTomlField(contents, sectionName, key, nextValue) {
  const lines = contents.split(/\r?\n/);
  let section = null;
  let updated = false;
  const next = lines.map((line) => {
    const sectionMatch = line.match(/^\[([^\]]+)\]/);
    if (sectionMatch) {
      section = sectionMatch[1];
      return line;
    }
    if (section === sectionName) {
      const match = line.match(new RegExp(`^(\\s*)${escapeRegExp(key)}\\s*=\\s*"`));
      if (match) {
        updated = true;
        return `${match[1]}${key} = "${nextValue.trim()}"`;
      }
    }
    return line;
  });
  if (!updated) {
    fail(`failed to update ${sectionName}.${key}`);
  }
  return `${next.join('\n')}\n`;
}

function updateTomlDependencyVersion(contents, sectionName, dependencyName, nextVersion) {
  const lines = contents.split(/\r?\n/);
  let section = null;
  let updated = false;
  const escapedDependency = escapeRegExp(dependencyName);
  const keyRe = new RegExp(`^(\\s*)${escapedDependency}\\s*=\\s*\\{([^}]*)\\}$`);
  const depRe = new RegExp(`(\\s*${escapedDependency}\\s*=\\s*\\{[^}]*\\bversion\\s*=\\s*")([^"]+)(")`);

  const next = lines.map((line) => {
    const sectionMatch = line.match(/^\[([^\]]+)\]/);
    if (sectionMatch) {
      section = sectionMatch[1];
      return line;
    }
    if (section === sectionName && keyRe.test(line)) {
      const match = line.match(depRe);
      if (!match) {
        fail(`unable to parse dependency entry for ${dependencyName}`);
      }
      updated = true;
      return line.replace(depRe, `$1${nextVersion}$3`);
    }
    return line;
  });
  if (!updated) {
    fail(`failed to update dependency ${dependencyName} in [${sectionName}]`);
  }
  return `${next.join('\n')}\n`;
}

function updateJsonField(contents, key, nextValue) {
  const escaped = escapeRegExp(`"${key}"`);
  const regex = new RegExp(`(${escaped}\\s*:\\s*")([^"]+)(")`);
  if (!regex.test(contents)) {
    fail(`failed to update ${key}`);
  }
  return contents.replace(regex, `$1${nextValue}$3`);
}

function readTomlStringValue(contents, sectionName, key) {
  const section = getTomlSection(contents, sectionName);
  const pattern = new RegExp(`^\\s*${escapeRegExp(key)}\\s*=\\s*"([^"]+)"`, 'm');
  const match = section.match(pattern);
  if (!match) {
    fail(`missing ${sectionName}.${key}`);
  }
  return match[1];
}

function getTomlSection(contents, sectionName) {
  const lines = contents.split(/\r?\n/);
  let current = null;
  const linesInSection = [];
  for (const line of lines) {
    const sectionMatch = line.match(/^\[([^\]]+)\]/);
    if (sectionMatch) {
      current = sectionMatch[1];
      continue;
    }
    if (current === sectionName) {
      linesInSection.push(line);
    }
  }
  if (linesInSection.length === 0) {
    fail(`missing section [${sectionName}]`);
  }
  return linesInSection.join('\n');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function printHelp() {
  console.log(`Usage: node scripts/bump-version.mjs [--targets core,cli,web] [--part patch|minor|major]`);
  console.log('Options:');
  console.log('  --targets <targets>   Comma separated list of components to bump');
  console.log('  --target <target>     Single target, repeatable');
  console.log('  --part <level>        patch|minor|major (default: patch)');
  console.log('  --help                Show this message');
}

function fail(message) {
  console.error(`[bump-version] ${message}`);
  process.exit(1);
}
