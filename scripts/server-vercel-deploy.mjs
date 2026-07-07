import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const isProduction = process.argv.includes('--prod');
const vercelToken = process.env.VERCEL_TOKEN;

if (!vercelToken) {
  throw new Error('VERCEL_TOKEN is required to deploy apps/server to Vercel');
}

const args = [
  '--yes',
  'vercel@latest',
  'deploy',
  '--project',
  'treease-server',
  '--token',
  vercelToken,
];

if (isProduction) {
  args.push('--prod');
}

const child = spawn('npx', args, {
  cwd: root,
  stdio: 'inherit',
  env: process.env,
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});
