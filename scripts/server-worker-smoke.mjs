import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const serverDir = path.join(root, 'apps', 'server');
const port = 4311;
const vars = {
  NODE_ENV: 'test',
  HOST: '127.0.0.1',
  PORT: String(port),
  APP_ORIGIN: 'https://treease.com',
  SUPABASE_URL: 'https://example.supabase.co',
  SUPABASE_ANON_KEY: 'anon-key',
  SUPABASE_SERVICE_ROLE_KEY: 'service-role-key',
  LEMONSQUEEZY_API_KEY: 'lemonsqueezy-api-key',
  LEMONSQUEEZY_STORE_ID: '1',
  LEMONSQUEEZY_WEBHOOK_SECRET: 'billing-secret',
  LEMONSQUEEZY_STORE_URL: 'https://billing.example.com',
  LEMONSQUEEZY_VARIANT_MAP: JSON.stringify({ monthly: 101, yearly: 102 }),
  AI_CREDENTIALS: JSON.stringify({ 'vercel-gateway-sonnet': ['gateway-key'], 'agnes-flash': ['agnes-key'] }),
};

const args = [
  'exec',
  'wrangler',
  'dev',
  '--config',
  'wrangler.jsonc',
  '--local',
  '--port',
  String(port),
  '--show-interactive-dev-session',
  'false',
  ...Object.entries(vars).flatMap(([key, value]) => ['--var', `${key}:${value}`]),
];
const child = spawn('pnpm', args, { cwd: serverDir, stdio: ['ignore', 'pipe', 'pipe'] });
let output = '';
child.stdout.on('data', (chunk) => { output += chunk.toString(); });
child.stderr.on('data', (chunk) => { output += chunk.toString(); });

try {
  const baseUrl = `http://127.0.0.1:${port}`;
  await waitForHealth(`${baseUrl}/health`, 15_000, child, () => output);
  await assertStatus(`${baseUrl}/health`, 200);
  await assertStatus(`${baseUrl}/v1/billing/plans`, 200);
  await assertStatus(`${baseUrl}/v1/auth/session`, 401);
  await assertStatus(`${baseUrl}/v1/public/shares/not-a-uuid`, 400);
  await assertStatus(`${baseUrl}/v1/billing/webhooks/lemonsqueezy`, 401, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-Signature': 'invalid' },
    body: '{}',
  });
  process.stdout.write('Cloudflare Worker smoke check passed\n');
} finally {
  child.kill('SIGTERM');
}

async function assertStatus(url, expectedStatus, init) {
  const response = await fetch(url, init);
  if (response.status !== expectedStatus) {
    throw new Error(`${url} returned ${response.status}; expected ${expectedStatus}`);
  }
}

async function waitForHealth(url, timeoutMs, processHandle, getOutput) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (processHandle.exitCode != null) throw new Error(`Worker exited early:\n${getOutput()}`);
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Timed out waiting for ${url}\n${getOutput()}`);
}
