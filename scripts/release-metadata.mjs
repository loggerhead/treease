import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const defaultRootDir = path.resolve(here, '..');

export function loadCoreReleaseMetadata(rootDir = defaultRootDir) {
  const coreManifestPath = path.resolve(rootDir, 'packages', 'core', 'Cargo.toml');
  const coreManifest = readFileSync(coreManifestPath, 'utf8');

  const coreName = readTomlString(coreManifest, 'package', 'name', coreManifestPath);
  const coreVersion = readTomlString(coreManifest, 'package', 'version', coreManifestPath);
  return {
    coreName,
    coreVersion,
  };
}

export function loadReleaseMetadata(rootDir = defaultRootDir) {
  const cliManifestPath = path.resolve(rootDir, 'apps', 'cli', 'Cargo.toml');
  const cliManifest = readFileSync(cliManifestPath, 'utf8');
  const { coreName, coreVersion } = loadCoreReleaseMetadata(rootDir);
  const webVersion = readPackageVersion(rootDir);

  if (webVersion !== coreVersion) {
    throw new Error(
      `apps/web/package.json version ${webVersion} does not match packages/core/Cargo.toml version ${coreVersion}`
    );
  }

  const cliName = readTomlString(cliManifest, 'package', 'name', cliManifestPath);
  const cliVersion = readTomlString(cliManifest, 'package', 'version', cliManifestPath);
  const cliCoreDependencyVersion = readInlineDependencyVersion(cliManifest, 'treease-core', cliManifestPath);

  if (cliCoreDependencyVersion !== coreVersion) {
    throw new Error(
      `apps/cli/Cargo.toml treease-core dependency version ${cliCoreDependencyVersion} does not match packages/core/Cargo.toml version ${coreVersion}`
    );
  }
  return {
    cliName,
    cliVersion,
    cliCoreDependencyVersion,
    coreName,
    coreVersion,
    webVersion,
    coreReleaseTag: `v${coreVersion}`,
    cliReleaseTag: `cli-v${cliVersion}`,
  };
}

function readPackageVersion(rootDir) {
  const manifestPath = path.resolve(rootDir, 'apps', 'web', 'package.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const version = manifest.version;
  if (typeof version !== 'string' || !/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`apps/web/package.json version must be a semver string, got ${version}`);
  }
  return version;
}

export function synchronizeGeneratedWasmPackageJson(packageJsonSource, { coreName, coreVersion }) {
  const pkg = JSON.parse(packageJsonSource);
  pkg.name = coreName;
  pkg.version = coreVersion;
  if (pkg.files) {
    pkg.files = pkg.files.map((file) => file.replace(/^core_bg\.wasm$/, 'core.wasm'));
  }
  return JSON.stringify(pkg, null, 2) + '\n';
}

function readTomlString(contents, sectionName, key, manifestPath) {
  const section = readTomlSection(contents, sectionName, manifestPath);
  const match = section.match(new RegExp(`^\\s*${escapeRegex(key)}\\s*=\\s*"([^"]+)"\\s*$`, 'm'));
  if (!match) {
    throw new Error(`missing ${sectionName}.${key} in ${path.relative(defaultRootDir, manifestPath)}`);
  }
  return match[1];
}

function readTomlSection(contents, sectionName, manifestPath) {
  const lines = contents.split(/\r?\n/);
  let currentSection = null;
  const sectionLines = [];
  for (const line of lines) {
    const sectionMatch = line.match(/^\s*\[([^\]]+)\]\s*$/);
    if (sectionMatch) {
      if (currentSection === sectionName) {
        break;
      }
      currentSection = sectionMatch[1];
      continue;
    }
    if (currentSection === sectionName) {
      sectionLines.push(line);
    }
  }
  if (sectionLines.length === 0) {
    throw new Error(`missing [${sectionName}] in ${path.relative(defaultRootDir, manifestPath)}`);
  }
  return sectionLines.join('\n');
}

function readInlineDependencyVersion(contents, dependencyName, manifestPath) {
  const dependencies = readTomlSection(contents, 'dependencies', manifestPath);
  const dependencyPattern = new RegExp(
    `^\\s*${escapeRegex(dependencyName)}\\s*=\\s*\\{[^\\n}]*\\bversion\\s*=\\s*"([^"]+)"[^\\n}]*\\}\\s*$`,
    'm'
  );
  const match = dependencies.match(dependencyPattern);
  if (!match) {
    throw new Error(
      `missing ${dependencyName} inline dependency version in ${path.relative(defaultRootDir, manifestPath)}`
    );
  }
  return match[1];
}

function escapeRegex(value) {
  return value.replace(/[|\\{}()[\]^$+*?.]/g, '\\$&');
}

async function main() {
  const rootDir = resolveArg('--root-dir') ?? defaultRootDir;
  const writeGithubOutput = hasArg('--github-output');
  const metadata = loadReleaseMetadata(rootDir);
  if (!writeGithubOutput) {
    process.stdout.write(`${JSON.stringify(metadata, null, 2)}\n`);
    return;
  }

  const output = Object.entries(metadata)
    .map(([key, value]) => `${key}=${value}`)
    .join('\n');
  process.stdout.write(`${output}\n`);
}

function resolveArg(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return null;
  return process.argv[index + 1] ?? null;
}

function hasArg(name) {
  return process.argv.includes(name);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((error) => {
    process.stderr.write(`[release-metadata] ${error.message}\n`);
    process.exit(1);
  });
}
