import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const serverDir = path.join(root, 'apps', 'server');
const tsx = path.join(serverDir, 'node_modules', 'tsx', 'dist', 'cli.mjs');

const port = 4310;
const env = {
  ...process.env,
  NODE_ENV: 'test',
  HOST: '127.0.0.1',
  PORT: String(port),
  APP_ORIGIN: 'https://treease.com',
  CORS_ORIGIN: 'https://treease.com',
  SUPABASE_URL: 'https://example.supabase.co',
  SUPABASE_ANON_KEY: 'anon-key',
  SUPABASE_SERVICE_ROLE_KEY: 'service-role-key',
  BILLING_WEBHOOK_SECRET: 'treease-sync-secret',
  LEMONSQUEEZY_WEBHOOK_SECRET: 'billing-secret',
  BILLING_CHECKOUT_URLS: JSON.stringify({
    'pro-monthly': 'https://billing.example.com/checkout/pro-monthly',
    'pro-yearly': 'https://billing.example.com/checkout/pro-yearly',
    'max-monthly': 'https://billing.example.com/checkout/max-monthly',
    'max-yearly': 'https://billing.example.com/checkout/max-yearly',
  }),
  BILLING_PORTAL_URL: 'https://billing.example.com/portal',
  LEMONSQUEEZY_API_KEY: 'lemonsqueezy-api-key',
  LEMONSQUEEZY_VARIANT_MAP: JSON.stringify({
    'pro-monthly': 101,
    'pro-yearly': 102,
    'max-monthly': 201,
    'max-yearly': 202,
  }),
  AI_GATEWAY_API_KEY: 'gateway-key',
  TREEASE_AI_MODEL: 'anthropic/claude-sonnet-4.5',
};

const child = spawn(process.execPath, [tsx, 'src/dev.ts'], {
  cwd: serverDir,
  env,
  stdio: ['ignore', 'pipe', 'pipe'],
});

let startupOutput = '';
child.stdout.on('data', (chunk) => {
  startupOutput += chunk.toString();
});
child.stderr.on('data', (chunk) => {
  startupOutput += chunk.toString();
});

try {
  await waitForHealth(`http://127.0.0.1:${port}/health`, 10_000);
  process.stdout.write('server smoke check passed\n');
} finally {
  child.kill('SIGTERM');
}

async function waitForHealth(url, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (child.exitCode != null) {
      throw new Error(`server exited early: ${startupOutput}`);
    }

    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {}

    await new Promise((resolve) => setTimeout(resolve, 200));
  }

  throw new Error(`timed out waiting for ${url}\n${startupOutput}`);
}
