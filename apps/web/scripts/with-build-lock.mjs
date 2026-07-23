import { readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const webDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const lockPath = path.join(webDir, '.svelte-kit-build.lock');
const command = process.argv.slice(2);

if (command.length === 0) {
  throw new Error('with-build-lock requires a command');
}

await acquireLock();

let released = false;
function releaseLock() {
  if (released) return;
  released = true;
  rmSync(lockPath, { force: true });
}

process.on('exit', releaseLock);
process.on('SIGINT', () => process.exit(130));
process.on('SIGTERM', () => process.exit(143));

const child = spawn(command[0], command.slice(1), {
  cwd: webDir,
  env: process.env,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

child.on('error', (error) => {
  releaseLock();
  throw error;
});

child.on('exit', (code, signal) => {
  releaseLock();
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exitCode = code ?? 1;
});

async function acquireLock() {
  while (true) {
    try {
      writeFileSync(lockPath, `${process.pid}\n`, { encoding: 'utf8', flag: 'wx' });
      return;
    } catch (error) {
      if (error?.code !== 'EEXIST') throw error;
      if (removeDeadLock()) continue;
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
}

function removeDeadLock() {
  try {
    const ownerPid = Number.parseInt(readFileSync(lockPath, 'utf8'), 10);
    if (!Number.isInteger(ownerPid) || ownerPid <= 0) {
      rmSync(lockPath, { force: true });
      return true;
    }
    process.kill(ownerPid, 0);
    return false;
  } catch (error) {
    if (error?.code === 'ESRCH' || error?.code === 'ENOENT') {
      rmSync(lockPath, { force: true });
      return true;
    }
    throw error;
  }
}
