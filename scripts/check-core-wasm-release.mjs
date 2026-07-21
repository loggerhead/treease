import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import process from 'node:process';

const CORE_MANIFEST = 'packages/core/Cargo.toml';
const CLI_MANIFEST = 'apps/cli/Cargo.toml';

function git(args) {
  return execFileSync('git', args, { encoding: 'utf8' }).trim();
}

function gitOutput(args) {
  return execFileSync('git', args, { encoding: 'utf8' });
}

function readReleaseDate(contents, file) {
  const section = contents.match(
    /\[package\.metadata\.treease\]([\s\S]*?)(?=\n\s*\[[^\]]+\]|\s*$)/,
  )?.[1];
  const value = section?.match(/^\s*wasm_release_date\s*=\s*"(\d{8})"\s*$/m)?.[1];
  if (!value) {
    throw new Error(`${file} 缺少有效的 package.metadata.treease.wasm_release_date`);
  }
  return value;
}

function changedFiles(mode) {
  const args = mode === 'staged'
    ? ['diff', '--cached', '--name-only', '--diff-filter=ACMRD', '-z']
    : mode === 'worktree'
      ? ['diff', 'HEAD', '--name-only', '--diff-filter=ACMRD', '-z']
      : ['diff', '--name-only', '--diff-filter=ACMRD', '-z', `${mode.base}...${mode.head}`];
  return gitOutput(args).split('\0').filter(Boolean);
}

function readBlob(mode, revision, file) {
  if (mode === 'staged' && revision === null) {
    return gitOutput(['show', `:${file}`]);
  }
  if (mode === 'worktree' && revision === undefined) {
    return readFileSync(file, 'utf8');
  }
  return gitOutput(['show', `${revision}:${file}`]);
}

function hasCoreChange(files) {
  return files.some((file) => file === 'packages/core' || file.startsWith('packages/core/'));
}

function check(mode) {
  const files = changedFiles(mode);
  const currentCore = readBlob(mode, mode === 'staged' ? null : mode.head, CORE_MANIFEST);
  const currentCli = readBlob(mode, mode === 'staged' ? null : mode.head, CLI_MANIFEST);
  const currentCoreDate = readReleaseDate(currentCore, CORE_MANIFEST);
  const currentCliDate = readReleaseDate(currentCli, CLI_MANIFEST);

  if (currentCoreDate !== currentCliDate) {
    throw new Error(
      `${CORE_MANIFEST} (${currentCoreDate}) 与 ${CLI_MANIFEST} (${currentCliDate}) 的 wasm_release_date 不一致`,
    );
  }

  if (!hasCoreChange(files)) return;

  if (mode === 'staged' || mode === 'worktree') {
    try {
      git(['rev-parse', '--verify', 'HEAD']);
    } catch {
      return;
    }
  }

  const base = mode === 'staged' || mode === 'worktree' ? git(['rev-parse', 'HEAD']) : mode.base;
  const previousCore = readBlob(mode, base, CORE_MANIFEST);
  const previousDate = readReleaseDate(previousCore, CORE_MANIFEST);
  if (previousDate === currentCoreDate) {
    throw new Error(
      `检测到 packages/core 发生变化，但 wasm_release_date 仍为 ${currentCoreDate}。请更新 packages/core/Cargo.toml，并同步 apps/cli/Cargo.toml。`,
    );
  }
}

function parseArgs(args) {
  if (args[0] === '--staged') return 'staged';
  if (args[0] === '--worktree') return 'worktree';
  const baseIndex = args.indexOf('--base');
  const headIndex = args.indexOf('--head');
  if (baseIndex !== -1 && headIndex !== -1 && args[baseIndex + 1] && args[headIndex + 1]) {
    return { base: args[baseIndex + 1], head: args[headIndex + 1] };
  }
  throw new Error('用法：--staged、--worktree，或 --base <revision> --head <revision>');
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    check(parseArgs(process.argv.slice(2)));
    process.stdout.write('Core WASM release version check passed.\n');
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}

export { check, readReleaseDate };
