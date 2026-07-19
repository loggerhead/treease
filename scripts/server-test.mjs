import { spawn } from 'node:child_process';
import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const serverDir = path.join(root, 'apps', 'server');
const testFiles = await findTestFiles(path.join(serverDir, 'src'));

await run(process.execPath, ['--import', 'tsx', '--test', ...testFiles], serverDir);

async function findTestFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...await findTestFiles(entryPath));
    } else if (entry.isFile() && entry.name.endsWith('.test.ts')) {
      files.push(path.relative(serverDir, entryPath));
    }
  }
  return files.sort();
}

function run(command, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      stdio: 'inherit',
      shell: false,
    });

    child.on('exit', (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} exited with code ${code ?? 1}`));
    });

    child.on('error', reject);
  });
}
