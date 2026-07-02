import { spawn } from 'node:child_process';
import { mkdir, stat, writeFile } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { loadCoreReleaseMetadata } from './release-metadata.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const defaultRootDir = path.resolve(here, '..');
const defaultHost = '127.0.0.1';
const defaultPort = 4317;

export async function resolveLocalCliAssetConfig({
  rootDir = defaultRootDir,
  host = defaultHost,
  port = defaultPort,
  runToken = `${Date.now()}-${process.pid}`,
} = {}) {
  const cliAssetsDir = path.resolve(rootDir, 'apps', 'web', 'build', 'cli-assets');
  const latestPath = path.join(cliAssetsDir, 'latest.json');
  const { coreWasmReleaseDate } = loadCoreReleaseMetadata(rootDir);
  const versionDir = path.join(cliAssetsDir, coreWasmReleaseDate);
  const manifestPath = path.join(versionDir, 'manifest.json');
  try {
    await stat(latestPath);
  } catch {
    throw new Error(
      `missing local cli-assets build output at ${latestPath}; run \`pnpm --dir apps/web build\` first`
    );
  }
  try {
    await stat(manifestPath);
  } catch {
    throw new Error(
      `missing local cli-assets bundle for wasm_release_date ${coreWasmReleaseDate} at ${manifestPath}; run \`pnpm --dir apps/web build\` to regenerate the matching bundle`
    );
  }

  return {
    rootDir,
    cliAssetsDir,
    latestPath,
    versionDir,
    manifestPath,
    wasmReleaseDate: coreWasmReleaseDate,
    assetBaseUrl: `http://${host}:${port}`,
    cacheDir: path.resolve(rootDir, '.tmp', `treease-web-local-${runToken}`),
    host,
    port,
  };
}

export async function startStaticFileServer({ rootDir, host = defaultHost, port = defaultPort }) {
  const server = http.createServer(async (request, response) => {
    try {
      const requestUrl = new URL(request.url ?? '/', `http://${host}:${port}`);
      const relativePath = decodeURIComponent(requestUrl.pathname).replace(/^\/+/, '');
      const filePath = resolveSafePath(rootDir, relativePath);
      if (!filePath) {
        response.writeHead(404);
        response.end('Not Found');
        return;
      }
      const fileInfo = await stat(filePath).catch(() => null);
      if (!fileInfo?.isFile()) {
        response.writeHead(404);
        response.end('Not Found');
        return;
      }

      response.writeHead(200, { 'content-type': contentTypeFor(filePath) });
      createReadStream(filePath).pipe(response);
    } catch (error) {
      response.writeHead(500);
      response.end(`Internal Server Error: ${error.message}`);
    }
  });

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, host, () => {
      server.off('error', reject);
      resolve();
    });
  });

  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('failed to resolve local cli-assets server address');
  }

  return {
    host,
    port: address.port,
    origin: `http://${host}:${address.port}`,
    async close() {
      await new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
    },
  };
}

export async function writeEnvFile(filePath, config) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(
    filePath,
    [
      `TREEASE_WEB_ASSET_BASE_URL=${config.assetBaseUrl}`,
      `TREEASE_WEB_CACHE_DIR=${config.cacheDir}`,
      '',
    ].join('\n'),
    'utf8'
  );
}

function resolveSafePath(rootDir, relativePath) {
  const requestedPath = relativePath === '' ? 'latest.json' : relativePath;
  const resolvedPath = path.resolve(rootDir, requestedPath);
  const relative = path.relative(rootDir, resolvedPath);
  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    return null;
  }
  return resolvedPath;
}

function contentTypeFor(filePath) {
  switch (path.extname(filePath)) {
    case '.html':
      return 'text/html; charset=utf-8';
    case '.js':
      return 'text/javascript; charset=utf-8';
    case '.json':
      return 'application/json; charset=utf-8';
    case '.css':
      return 'text/css; charset=utf-8';
    case '.wasm':
      return 'application/wasm';
    default:
      return 'application/octet-stream';
  }
}

async function runServeCommand() {
  const config = await resolveLocalCliAssetConfig({
    rootDir: resolveArg('--root-dir') ?? defaultRootDir,
    port: Number(resolveArg('--port') ?? defaultPort),
  });
  const server = await startStaticFileServer({
    rootDir: config.cliAssetsDir,
    host: config.host,
    port: config.port,
  });
  const activeConfig = { ...config, assetBaseUrl: server.origin, port: server.port };
  const envFile = resolveArg('--env-file');
  if (envFile) {
    await writeEnvFile(envFile, activeConfig);
  }

  process.stdout.write(`${server.origin}\n`);
  process.stdout.write(`TREEASE_WEB_ASSET_BASE_URL=${server.origin}\n`);
  process.stdout.write(`TREEASE_WEB_CACHE_DIR=${activeConfig.cacheDir}\n`);
}

async function runTreeaseWebCommand() {
  const forwardedArgs = readForwardedArgs();
  const config = await resolveLocalCliAssetConfig({
    rootDir: resolveArg('--root-dir') ?? defaultRootDir,
    port: Number(resolveArg('--port') ?? defaultPort),
  });
  await mkdir(config.cacheDir, { recursive: true });
  const server = await startStaticFileServer({
    rootDir: config.cliAssetsDir,
    host: config.host,
    port: config.port,
  });

  const child = spawn(
    'cargo',
    ['run', '--locked', '--manifest-path', 'apps/cli/Cargo.toml', '--bin', 'treease', '--', 'web', ...forwardedArgs],
    {
      cwd: config.rootDir,
      stdio: 'inherit',
      env: {
        ...process.env,
        TREEASE_WEB_ASSET_BASE_URL: server.origin,
        TREEASE_WEB_CACHE_DIR: config.cacheDir,
      },
    }
  );

  const closeServer = async () => {
    await server.close().catch(() => {});
  };
  process.on('SIGINT', async () => {
    child.kill('SIGINT');
    await closeServer();
  });
  process.on('SIGTERM', async () => {
    child.kill('SIGTERM');
    await closeServer();
  });

  const exitCode = await new Promise((resolve) => {
    child.on('error', (error) => {
      process.stderr.write(`[treease-web-local] failed to start cargo: ${error.message}\n`);
      resolve(1);
    });
    child.on('exit', (code, signal) => {
      resolve(signal ? 1 : (code ?? 0));
    });
  });
  await closeServer();
  process.exit(exitCode);
}

function readForwardedArgs() {
  const separator = process.argv.indexOf('--');
  return separator === -1 ? process.argv.slice(2) : process.argv.slice(separator + 1);
}

function resolveArg(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return null;
  return process.argv[index + 1] ?? null;
}

async function main() {
  const mode = process.argv[2];
  if (mode === 'serve') {
    await runServeCommand();
    return;
  }
  await runTreeaseWebCommand();
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((error) => {
    process.stderr.write(`[treease-web-local] ${error.message}\n`);
    process.exit(1);
  });
}
