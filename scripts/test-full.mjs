import { createHash } from 'node:crypto';
import { execFile, spawn } from 'node:child_process';
import { mkdir, readFile, readlink, writeFile, lstat } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

const execFileAsync = promisify(execFile);
const rootDir = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const statePath = path.join(rootDir, '.cache', 'treease-test-state', 'core.json');
const cargoTargetDir = path.resolve(
  rootDir,
  process.env.CARGO_TARGET_DIR || 'packages/core/target',
);
const cargoTestEnv = { ...process.env, CARGO_TARGET_DIR: cargoTargetDir };

const forceCoreTest = process.env.TREEASE_FORCE_CORE_TEST === '1';
const input = await computeCoreTestInput();
const state = await readState();

if (forceCoreTest || !isReusable(state, input)) {
  await run('pnpm', ['run', 'core:test'], rootDir, cargoTestEnv);
  // Persist only after Core passes; a failed run must force the next full test to retry it.
  await mkdir(path.dirname(statePath), { recursive: true });
  await writeFile(statePath, `${JSON.stringify(input, null, 2)}\n`);
} else {
  console.log('Skip Core tests: source and test inputs are unchanged since the last successful run.');
}

await run(
  'cargo',
  ['nextest', 'run', '--locked', '--lib'],
  path.join(rootDir, 'apps', 'cli'),
  cargoTestEnv,
);
await run('bash', ['tests/acceptance/run.sh'], path.join(rootDir, 'apps', 'cli'), cargoTestEnv);
await run('pnpm', ['test'], path.join(rootDir, 'apps', 'web'));
await run('pnpm', ['test:e2e:cloudflare'], path.join(rootDir, 'apps', 'web'));

async function computeCoreTestInput() {
  const files = await effectiveCoreFiles();
  const inputFiles = [...files, 'package.json', 'scripts/test-full.mjs'].sort();
  const hash = createHash('sha256');

  for (const relativePath of inputFiles) {
    const absolutePath = path.join(rootDir, relativePath);
    const stats = await lstat(absolutePath);
    hash.update(relativePath);
    hash.update('\0');

    if (stats.isSymbolicLink()) {
      hash.update('symlink\0');
      hash.update(await readlink(absolutePath));
    } else if (stats.isFile()) {
      hash.update('file\0');
      hash.update(await readFile(absolutePath));
    } else {
      throw new Error(`Unsupported Core test input: ${relativePath}`);
    }

    hash.update('\0');
  }

  return {
    fingerprint: hash.digest('hex'),
    files,
    environment: {
      platform: process.platform,
      arch: process.arch,
      rustc: await commandVersion('rustc', ['-Vv']),
      nextest: await commandVersion('cargo', ['nextest', '--version']),
    },
  };
}

async function effectiveCoreFiles() {
  const listed = splitNull(await git(['ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', 'packages/core']));
  // --no-index applies ignore rules to tracked files too, which Git normally keeps visible.
  const ignored = new Set(splitNull(await git(['check-ignore', '--no-index', '--stdin', '-z'], listed.join('\0'))));
  const files = listed.filter((file) => !ignored.has(file));

  for (const file of listed) {
    if (file.endsWith('/.gitignore')) files.push(file);
  }

  files.push('.gitignore');
  return [...new Set(files)].sort();
}

function isReusable(state, input) {
  return state?.fingerprint === input.fingerprint &&
    sameEnvironment(state.environment, input.environment);
}

function sameEnvironment(left, right) {
  return left?.platform === right.platform &&
    left?.arch === right.arch &&
    left?.rustc === right.rustc &&
    left?.nextest === right.nextest;
}

async function readState() {
  try {
    return JSON.parse(await readFile(statePath, 'utf8'));
  } catch (error) {
    if (error.code === 'ENOENT' || error instanceof SyntaxError) return null;
    throw error;
  }
}

async function commandVersion(command, args) {
  const { stdout } = await execFileAsync(command, args, { cwd: rootDir });
  return stdout.trim();
}

async function git(args, input = '') {
  if (!input) {
    const { stdout } = await execFileAsync('git', args, {
      cwd: rootDir,
      maxBuffer: 16 * 1024 * 1024,
    });
    return stdout;
  }

  return new Promise((resolve, reject) => {
    const child = spawn('git', args, { cwd: rootDir, stdio: ['pipe', 'pipe', 'pipe'], shell: false });
    const stdout = [];
    const stderr = [];
    child.stdout.on('data', (chunk) => stdout.push(chunk));
    child.stderr.on('data', (chunk) => stderr.push(chunk));
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0 || (args[0] === 'check-ignore' && code === 1)) {
        resolve(Buffer.concat(stdout).toString('utf8'));
      } else {
        reject(new Error(`git ${args.join(' ')} exited with code ${code ?? 1}: ${Buffer.concat(stderr)}`));
      }
    });
    child.stdin.end(input);
  });
}

function splitNull(value) {
  return value.split('\0').filter(Boolean);
}

function run(command, args, cwd, env = process.env) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, env, stdio: 'inherit', shell: false });
    child.on('exit', (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} exited with code ${code ?? 1}`));
    });
    child.on('error', reject);
  });
}
