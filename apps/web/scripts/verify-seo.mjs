import { access, readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const webDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const buildDir = path.resolve(webDir, process.env.SEO_BUILD_DIR ?? '.svelte-kit/cloudflare');
const sitemapPath = path.join(webDir, 'static', 'sitemap.xml');
const robotsPath = path.join(webDir, 'static', 'robots.txt');

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await walk(file)));
    else files.push(file);
  }
  return files;
}

function fail(message) {
  throw new Error(`[seo] ${message}`);
}

function attrs(tag) {
  return Object.fromEntries(
    [...tag.matchAll(/([\w:-]+)=(?:"([^"]*)"|'([^']*)')/g)].map((match) => [match[1], match[2] ?? match[3] ?? '']),
  );
}

function metaValue(html, key, value) {
  const tag = [...html.matchAll(/<meta\s+[^>]*>/gi)]
    .map((match) => match[0])
    .find((candidate) => attrs(candidate)[key] === value);
  return tag ? attrs(tag).content : null;
}

function pagePathFromHtml(file) {
  const relative = path.relative(buildDir, file).replaceAll(path.sep, '/');
  if (relative === 'index.html') return '/';
  return `/${relative.slice(0, -'.html'.length)}`;
}

function htmlFileForPath(pagePath) {
  return pagePath === '/' ? path.join(buildDir, 'index.html') : path.join(buildDir, `${pagePath.slice(1)}.html`);
}

const htmlFiles = (await walk(buildDir)).filter((file) => file.endsWith('.html'));
const nonPublicHtmlFiles = new Set(['200.html', '404.html', 'editor.html', 'auth/callback.html']);
const publicHtmlFiles = htmlFiles.filter((file) => !nonPublicHtmlFiles.has(path.relative(buildDir, file)));
const sitemap = await readFile(sitemapPath, 'utf8');
const sitemapPaths = [...sitemap.matchAll(/<loc>(https?:\/\/[^<]+)<\/loc>/g)].map((match) => new URL(match[1]).pathname);
const publicPaths = publicHtmlFiles.map(pagePathFromHtml).sort();

if (sitemapPaths.length !== new Set(sitemapPaths).size) fail('Sitemap contains duplicate URLs');
if (JSON.stringify([...new Set(sitemapPaths)].sort()) !== JSON.stringify(publicPaths)) {
  fail(`Sitemap and built public routes differ. Sitemap=${sitemapPaths.length}, build=${publicPaths.length}`);
}

const robots = await readFile(robotsPath, 'utf8');
const robotsSitemaps = [...robots.matchAll(/^Sitemap:\s*(\S+)\s*$/gim)].map((match) => match[1]);
if (robotsSitemaps.length !== 1 || robotsSitemaps[0] !== 'https://treease.com/sitemap.xml') {
  fail('robots.txt must declare exactly https://treease.com/sitemap.xml');
}
if (!/^User-agent:\s*\*\s*$/im.test(robots) || !/^Allow:\s*\/\s*$/im.test(robots)) {
  fail('robots.txt must allow public crawling for the default user agent');
}

for (const pagePath of publicPaths) {
  const file = htmlFileForPath(pagePath);
  const html = await readFile(file, 'utf8');
  const title = html.match(/<title>([\s\S]*?)<\/title>/i)?.[1]?.trim();
  const canonical = [...html.matchAll(/<link\s+[^>]*>/gi)]
    .map((match) => attrs(match[0]))
    .find((candidate) => candidate.rel === 'canonical')?.href;
  if (!title || !metaValue(html, 'name', 'description') || !canonical) fail(`${pagePath} is missing title, description, or canonical`);
  for (const [key, value] of [
    ['property', 'og:type'],
    ['property', 'og:title'],
    ['property', 'og:description'],
    ['property', 'og:url'],
    ['property', 'og:image'],
    ['name', 'twitter:card'],
    ['name', 'twitter:title'],
    ['name', 'twitter:description'],
    ['name', 'twitter:image'],
  ]) {
    if (!metaValue(html, key, value)) fail(`${pagePath} is missing ${value}`);
  }

  if (pagePath === '/' || pagePath.startsWith('/tutorial/')) {
    const scripts = [...html.matchAll(/<script\s+type="application\/ld\+json">([\s\S]*?)<\/script>/gi)].map((match) => match[1].trim());
    if (scripts.length === 0) fail(`${pagePath} is missing JSON-LD`);
    for (const script of scripts) {
      try {
        JSON.parse(script);
      } catch (error) {
        fail(`${pagePath} contains invalid JSON-LD: ${error.message}`);
      }
    }
  }
}

for (const relative of ['editor.html', 'auth/callback.html']) {
  let html = '';
  try {
    html = await readFile(path.join(buildDir, relative), 'utf8');
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const headers = await readFile(path.join(webDir, '_headers'), 'utf8');
  const routePattern = relative === 'editor.html' ? /\/editor\*[\s\S]*?X-Robots-Tag:\s*noindex/i : /\/auth\/callback\*[\s\S]*?X-Robots-Tag:\s*noindex/i;
  if (!html && !routePattern.test(headers)) fail(`${relative} must remain noindex through headers`);
  if (html && !/noindex/i.test(html) && !routePattern.test(headers)) fail(`${relative} must remain noindex`);
  if (sitemapPaths.includes(pagePathFromHtml(path.join(buildDir, relative)))) fail(`${relative} must not be in Sitemap`);
}

const notFoundHtml = await readFile(path.join(buildDir, '404.html'), 'utf8');
if (!/<meta\s+name="robots"\s+content="noindex"\s*\/?>/i.test(notFoundHtml)) {
  fail('404.html must remain noindex');
}

const knownRoutePaths = new Set([...publicPaths, '/editor', '/auth/callback']);
async function hasLocalPath(pagePath) {
  if (knownRoutePaths.has(pagePath)) return true;
  const candidates = [
    pagePath === '/' ? path.join(buildDir, 'index.html') : path.join(buildDir, `${pagePath.slice(1)}.html`),
    path.join(buildDir, pagePath.slice(1)),
  ];
  for (const candidate of candidates) {
    try {
      await access(candidate);
      return true;
    } catch {
      // Try the next extensionless or generated file representation.
    }
  }
  return false;
}

const brokenLinks = [];
for (const file of publicHtmlFiles) {
  const html = await readFile(file, 'utf8');
  for (const match of html.matchAll(/<a\s+[^>]*href=(?:"([^"]*)"|'([^']*)')/gi)) {
    const href = match[1] ?? match[2] ?? '';
    if (!href || href.startsWith('#') || /^(?:mailto|tel|javascript):/i.test(href)) continue;
    let url;
    try {
      url = new URL(href, 'https://treease.com');
    } catch {
      brokenLinks.push(`${pagePathFromHtml(file)} -> ${href} (invalid URL)`);
      continue;
    }
    if (url.origin !== 'https://treease.com') continue;
    const linkedPath = url.pathname.replace(/\/\/+/g, '/') || '/';
    if (!(await hasLocalPath(linkedPath))) brokenLinks.push(`${pagePathFromHtml(file)} -> ${href}`);
  }
}
if (brokenLinks.length) fail(`local links point to missing routes: ${brokenLinks.join(', ')}`);

const assetFiles = (await walk(path.join(buildDir, '_app'))).filter((file) => file.endsWith('.js'));
const oversized = [];
for (const file of assetFiles) {
  const size = (await readFile(file)).byteLength;
  if (size > 700 * 1024) oversized.push(`${path.relative(buildDir, file)}=${size} bytes`);
}
if (oversized.length) fail(`client asset budget exceeded: ${oversized.join(', ')}`);

const homepage = await readFile(path.join(buildDir, 'index.html'), 'utf8');
if (!homepage.includes('https://x.com/1oggerhead') || !homepage.includes('https://discord.gg/vzM3Jdvav')) {
  fail('social profile links are missing from the rendered site');
}

console.log(`[seo] verified ${publicPaths.length} public HTML routes, local links, JSON-LD, social metadata, robots/Sitemap/noindex parity, and client asset budget`);
