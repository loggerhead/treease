import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const serverDir = path.join(root, 'apps', 'server');

const requiredFiles = [
  'src/app.ts',
  'src/routes/auth.ts',
  'src/routes/billing.ts',
  'src/routes/share.ts',
  'src/routes/ai.ts',
  'src/routes/usage.ts',
  'src/worker.ts',
  'src/worker-app.ts',
  'wrangler.jsonc',
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
assert(packageJson.dependencies?.ai, 'apps/server must depend on the AI SDK (ai)');
assert(packageJson.dependencies?.['@supabase/supabase-js'], 'apps/server must depend on @supabase/supabase-js');

const workerConfig = readJson(path.join(serverDir, 'wrangler.jsonc'));
assert(workerConfig.name === 'treease-server', 'apps/server/wrangler.jsonc must name the Worker treease-server');
assert(workerConfig.main === 'src/worker.ts', 'apps/server/wrangler.jsonc must use src/worker.ts as its entrypoint');
assert(
  workerConfig.compatibility_flags?.includes('nodejs_compat'),
  'apps/server/wrangler.jsonc must enable nodejs_compat',
);
assert(packageJson.devDependencies?.wrangler, 'apps/server must depend on Wrangler');
assert(packageJson.devDependencies?.['@cloudflare/workers-types'], 'apps/server must depend on Cloudflare Worker types');

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
  'LEMONSQUEEZY_API_KEY',
  'LEMONSQUEEZY_STORE_ID',
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
  'LEMONSQUEEZY_API_KEY',
  'LEMONSQUEEZY_STORE_ID',
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

for (const resourceType of ['compare', 'text_snapshot']) {
  assert(schemaSource.includes(resourceType), `Supabase schema must support share resource type ${resourceType}`);
}
assert(schemaSource.includes("feature in ('suggest_yq')"), 'Supabase schema must constrain usage_ledger to suggest_yq');

const appSource = readFile(path.join(serverDir, 'src', 'app.ts'));
assert(appSource.includes('preParsing'), 'src/app.ts must preserve raw request bodies for webhook verification');
assert(appSource.includes('request.rawBody'), 'src/app.ts must attach request.rawBody');

const workerSource = readFile(path.join(serverDir, 'src', 'worker.ts'));
const workerAppSource = readFile(path.join(serverDir, 'src', 'worker-app.ts'));
assert(workerSource.includes('workerApp'), 'src/worker.ts must export the Worker application');
assert(workerAppSource.includes('new Hono'), 'src/worker-app.ts must use a fetch-native Worker router');

const billingRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'billing.ts'));
for (const routePath of [
  '/v1/billing/plans',
  '/v1/billing/subscription',
  '/v1/billing/pricing-prewarm',
  '/v1/billing/checkout-link',
  '/v1/billing/portal-link',
  '/v1/billing/webhooks/lemonsqueezy',
]) {
  assert(billingRoutesSource.includes(routePath), `billing routes must expose ${routePath}`);
}

const aiRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'ai.ts'));
assert(aiRoutesSource.includes('/v1/ai/suggest-yq'), 'AI routes must expose /v1/ai/suggest-yq');

const usageRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'usage.ts'));
for (const routePath of ['/v1/usage', '/v1/usage/event-counts', '/v1/usage/events']) {
  assert(usageRoutesSource.includes(routePath), `usage routes must expose ${routePath}`);
}

const shareRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'share.ts'));
assert(shareRoutesSource.includes('/v1/share-links'), 'share routes must expose /v1/share-links');

const publicRoutesSource = readFile(path.join(serverDir, 'src', 'routes', 'public.ts'));
assert(publicRoutesSource.includes('/v1/public/shares/:shareID'), 'public routes must expose /v1/public/shares/:shareID');
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
