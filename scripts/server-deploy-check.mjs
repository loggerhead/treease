import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const serverDir = path.join(root, 'apps', 'server');

const requiredFiles = [
  'api/index.ts',
  'src/app.ts',
  'src/routes/auth.ts',
  'src/routes/billing.ts',
  'src/routes/share.ts',
  'src/routes/ai.ts',
  'src/routes/usage.ts',
  'vercel.json',
  '.env.example',
  'README.md',
  'supabase/0001_treease_server.sql',
];

for (const relativePath of requiredFiles) {
  const absolutePath = path.join(serverDir, relativePath);
  assert(existsSync(absolutePath), `missing required file: apps/server/${relativePath}`);
}

const packageJson = readJson(path.join(serverDir, 'package.json'));
assert(packageJson.name === 'treease-server', 'apps/server/package.json name must be treease-server');
assert(packageJson.dependencies?.fastify, 'apps/server must depend on fastify');
assert(packageJson.dependencies?.ai, 'apps/server must depend on Vercel AI SDK (ai)');
assert(packageJson.dependencies?.['@supabase/supabase-js'], 'apps/server must depend on @supabase/supabase-js');

const vercelConfig = readJson(path.join(serverDir, 'vercel.json'));
assert(vercelConfig.framework === null, 'apps/server/vercel.json must set framework to null');
assert(vercelConfig.fluid === true, 'apps/server/vercel.json must enable fluid compute');
assert(
  vercelConfig.functions?.['api/index.ts']?.maxDuration >= 30,
  'apps/server/vercel.json must set api/index.ts maxDuration >= 30',
);
assert(
  Array.isArray(vercelConfig.rewrites) &&
    vercelConfig.rewrites.some((rewrite) => rewrite.source === '/(.*)' && rewrite.destination === '/api'),
  'apps/server/vercel.json must rewrite all requests to /api',
);

const envExample = readFile(path.join(serverDir, '.env.example'));
const envKeys = new Set(
  envExample
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith('#') && line.includes('='))
    .map((line) => line.slice(0, line.indexOf('='))),
);

for (const key of [
  'HOST',
  'PORT',
  'APP_ORIGIN',
  'SUPABASE_URL',
  'SUPABASE_ANON_KEY',
  'SUPABASE_SERVICE_ROLE_KEY',
  'APP_ORIGIN',
  'BILLING_WEBHOOK_SECRET',
  'LEMONSQUEEZY_WEBHOOK_SECRET',
  'LEMONSQUEEZY_STORE_URL',
  'LEMONSQUEEZY_VARIANT_MAP',
  'AI_CREDENTIALS',
]) {
  assert(envKeys.has(key), `.env.example must include ${key}`);
}

const envSource = readFile(path.join(serverDir, 'src', 'env.ts'));
for (const key of [
  'SUPABASE_URL',
  'SUPABASE_ANON_KEY',
  'SUPABASE_SERVICE_ROLE_KEY',
  'BILLING_WEBHOOK_SECRET',
  'LEMONSQUEEZY_WEBHOOK_SECRET',
  'LEMONSQUEEZY_STORE_URL',
  'LEMONSQUEEZY_VARIANT_MAP',
  'AI_CREDENTIALS',
]) {
  assert(envSource.includes(key), `src/env.ts must validate ${key}`);
}

const schemaSource = readFile(path.join(serverDir, 'supabase', '0001_treease_server.sql'));
for (const tableName of ['subscriptions', 'share_links', 'usage_ledger']) {
  assert(
    schemaSource.includes(`create table if not exists public.${tableName}`),
    `Supabase schema must create ${tableName}`,
  );
  assert(
    schemaSource.includes(`alter table public.${tableName} enable row level security;`),
    `Supabase schema must enable RLS for ${tableName}`,
  );
}

for (const resourceType of ['editor_text_snapshot', 'command_run']) {
  assert(schemaSource.includes(resourceType), `Supabase schema must support share resource type ${resourceType}`);
}
assert(schemaSource.includes("feature in ('suggest_yq')"), 'Supabase schema must constrain usage_ledger to suggest_yq');

const appSource = readFile(path.join(serverDir, 'src', 'app.ts'));
assert(appSource.includes('preParsing'), 'src/app.ts must preserve raw request bodies for webhook verification');
assert(appSource.includes('request.rawBody'), 'src/app.ts must attach request.rawBody');

const billingRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'billing.ts'));
for (const routePath of [
  '/v1/billing/plans',
  '/v1/billing/subscription',
  '/v1/billing/checkout-link',
  '/v1/billing/portal-link',
  '/v1/billing/webhooks/subscription-sync',
  '/v1/billing/webhooks/lemonsqueezy',
]) {
  assert(billingRoutesSource.includes(routePath), `billing routes must expose ${routePath}`);
}

const aiRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'ai.ts'));
assert(aiRoutesSource.includes('/v1/ai/suggest-yq'), 'AI routes must expose /v1/ai/suggest-yq');

const usageRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'usage.ts'));
assert(usageRoutesSource.includes('/v1/usage/credits'), 'usage routes must expose /v1/usage/credits');

const shareRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'share.ts'));
assert(shareRoutesSource.includes('/v1/share-links'), 'share routes must expose /v1/share-links');

const publicRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'public.ts'));
assert(publicRoutesSource.includes('/v1/public/shares/:slug'), 'public routes must expose /v1/public/shares/:slug');
assert(publicRoutesSource.includes('/health'), 'public routes must expose /health');

process.stdout.write('server deploy check passed\n');

function readFile(filePath) {
  return readFileSync(filePath, 'utf8');
}

function readJson(filePath) {
  return JSON.parse(readFile(filePath));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}
