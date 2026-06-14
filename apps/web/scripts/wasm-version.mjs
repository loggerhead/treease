import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const rootDir = path.resolve(webDir, '..', '..');
const coreDir = path.resolve(rootDir, 'packages', 'core');
const versionFile = path.resolve(coreDir, 'output', 'core-web.version');

function readVersionFile() {
  if (!existsSync(versionFile)) return null;
  const value = readFileSync(versionFile, 'utf8').trim();
  return value || null;
}

function runGit(args) {
  try {
    return execFileSync('git', args, {
      cwd: coreDir,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore']
    }).trim();
  } catch {
    return null;
  }
}

function buildTimestamp() {
  const commitTime = gitCommitTimeIfClean();
  if (commitTime) return commitTime;
  const now = new Date();
  return `${pad2(now.getHours())}${pad2(now.getMinutes())}${pad2(now.getSeconds())}`;
}

function pad2(value) {
  return String(value).padStart(2, '0');
}

function gitCommitTimeIfClean() {
  const status = runGit(['status', '--porcelain']);
  if (status === null || status.trim() !== '') return null;
  return runGit(['log', '-1', '--format=%cd', '--date=format:%H%M%S']);
}

function resolveVersion() {
  const fromFile = readVersionFile();
  if (fromFile) return fromFile;
  return buildTimestamp();
}

process.stdout.write(resolveVersion());
