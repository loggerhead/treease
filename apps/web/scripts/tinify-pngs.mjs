import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { readFileSync } from 'node:fs';
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(here, '..');
const envPath = path.resolve(webDir, '.env.local');
const manifestPath = path.resolve(webDir, '.tinify-manifest.json');
const roots = [path.resolve(webDir, 'static')];

function loadEnvFile(filePath) {
  if (!existsSync(filePath)) return;
  const text = requireText(filePath);
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const eq = trimmed.indexOf('=');
    if (eq <= 0) continue;
    const key = trimmed.slice(0, eq).trim();
    const value = trimmed.slice(eq + 1).trim();
    if (!(key in process.env)) {
      process.env[key] = value;
    }
  }
}

function requireText(filePath) {
  return readFileSync(filePath, 'utf8');
}

async function readManifest() {
  if (!existsSync(manifestPath)) return {};
  try {
    return JSON.parse(await readFile(manifestPath, 'utf8'));
  } catch {
    return {};
  }
}

async function writeManifest(manifest) {
  await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + '\n', 'utf8');
}

async function walkPngFiles(rootDir, files = []) {
  const entries = await readdir(rootDir, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue;
    const nextPath = path.resolve(rootDir, entry.name);
    if (entry.isDirectory()) {
      await walkPngFiles(nextPath, files);
      continue;
    }
    if (!entry.isFile()) continue;
    if (path.extname(entry.name).toLowerCase() !== '.png') continue;
    files.push(nextPath);
  }
  return files;
}

function toSha256(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

async function compressWithTinify(apiKey, buffer) {
  const auth = Buffer.from(`api:${apiKey}`).toString('base64');
  const shrinkResponse = await fetch('https://api.tinify.com/shrink', {
    method: 'POST',
    headers: {
      Authorization: `Basic ${auth}`,
      'Content-Type': 'application/octet-stream',
    },
    body: buffer,
  });

  if (!shrinkResponse.ok) {
    const body = await shrinkResponse.text();
    throw new Error(`Tinify shrink failed (${shrinkResponse.status}): ${body}`);
  }

  const outputUrl = shrinkResponse.headers.get('location');
  if (!outputUrl) {
    throw new Error('Tinify shrink succeeded without output location');
  }

  const outputResponse = await fetch(outputUrl, {
    headers: {
      Authorization: `Basic ${auth}`,
    },
  });

  if (!outputResponse.ok) {
    const body = await outputResponse.text();
    throw new Error(`Tinify download failed (${outputResponse.status}): ${body}`);
  }

  return Buffer.from(await outputResponse.arrayBuffer());
}

function toRelative(targetPath) {
  return path.relative(webDir, targetPath).replaceAll(path.sep, '/');
}

async function main() {
  loadEnvFile(envPath);
  const apiKey = process.env.TINIFY_API_KEY;
  if (!apiKey) {
    throw new Error(`TINIFY_API_KEY is missing. Expected it in ${envPath}`);
  }

  const manifest = await readManifest();
  const files = [];
  for (const root of roots) {
    if (!existsSync(root)) continue;
    await walkPngFiles(root, files);
  }
  files.sort((a, b) => toRelative(a).localeCompare(toRelative(b)));

  const optimizedBySourceHash = new Map();
  let compressed = 0;
  let reused = 0;
  let skipped = 0;
  let inputBytes = 0;
  let outputBytes = 0;

  for (const filePath of files) {
    const relativePath = toRelative(filePath);
    const original = await readFile(filePath);
    const originalHash = toSha256(original);
    const currentBytes = original.byteLength;
    inputBytes += currentBytes;

    const prior = manifest[relativePath];
    if (prior?.sha256 === originalHash) {
      skipped += 1;
      outputBytes += prior.outputBytes ?? currentBytes;
      process.stdout.write(`[tinify] skip ${relativePath}\n`);
      continue;
    }

    const reusedBuffer = optimizedBySourceHash.get(originalHash);
    if (reusedBuffer) {
      await writeFile(filePath, reusedBuffer);
      const optimizedHash = toSha256(reusedBuffer);
      manifest[relativePath] = {
        sha256: optimizedHash,
        sourceHash: originalHash,
        inputBytes: currentBytes,
        outputBytes: reusedBuffer.byteLength,
        optimizedAt: new Date().toISOString(),
      };
      reused += 1;
      outputBytes += reusedBuffer.byteLength;
      process.stdout.write(`[tinify] reuse ${relativePath}\n`);
      continue;
    }

    process.stdout.write(`[tinify] compress ${relativePath}\n`);
    const optimized = await compressWithTinify(apiKey, original);
    await writeFile(filePath, optimized);
    optimizedBySourceHash.set(originalHash, optimized);
    const optimizedHash = toSha256(optimized);
    manifest[relativePath] = {
      sha256: optimizedHash,
      sourceHash: originalHash,
      inputBytes: currentBytes,
      outputBytes: optimized.byteLength,
      optimizedAt: new Date().toISOString(),
    };
    compressed += 1;
    outputBytes += optimized.byteLength;
  }

  await writeManifest(manifest);

  const savedBytes = inputBytes - outputBytes;
  const savedPct = inputBytes > 0 ? ((savedBytes / inputBytes) * 100).toFixed(1) : '0.0';
  process.stdout.write(
    `[tinify] done files=${files.length} compressed=${compressed} reused=${reused} skipped=${skipped} saved=${savedBytes}B (${savedPct}%)\n`,
  );
}

await mkdir(path.dirname(manifestPath), { recursive: true });
await main().catch((error) => {
  process.stderr.write(`[tinify] ${error.message}\n`);
  process.exit(1);
});
