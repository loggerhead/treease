import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const routesRoot = path.join(repoRoot, 'apps/web/src/routes');
const sitemapPath = path.join(repoRoot, 'apps/web/static/sitemap.xml');
const siteOrigin = 'https://treease.com';

const excludedRouteSegments = new Set(['auth', 'editor']);

async function findPageRoutes(directory, segments = []) {
  const entries = await fs.readdir(directory, { withFileTypes: true });
  const routes = [];

  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      routes.push(...(await findPageRoutes(entryPath, [...segments, entry.name])));
      continue;
    }
    if (entry.name !== '+page.svelte') continue;

    const routeSegments = segments.filter((segment) => !segment.startsWith('[') && !segment.startsWith('('));
    if (routeSegments.some((segment) => excludedRouteSegments.has(segment))) continue;
    routes.push(`/${routeSegments.join('/')}`.replace(/\/$/, '') || '/');
  }

  return routes;
}

async function findTutorialSlugs() {
  const source = await fs.readFile(
    path.join(repoRoot, 'apps/web/src/lib/content/tutorials/index.ts'),
    'utf8',
  );
  return [...source.matchAll(/\bslug:\s*'([^']+)'/g)].map((match) => match[1]);
}

async function findChangelogSlugs() {
  const source = await fs.readFile(
    path.join(repoRoot, 'apps/web/src/lib/content/changelog.generated.ts'),
    'utf8',
  );
  return [...source.matchAll(/"slug":\s*"([^\"]+)"/g)].map((match) => match[1]);
}

function toXmlUrl(url) {
  return `  <url><loc>${siteOrigin}${url}</loc></url>`;
}

const routeUrls = await findPageRoutes(routesRoot);
const tutorialUrls = (await findTutorialSlugs()).map((slug) => `/tutorial/${slug}`);
const changelogUrls = (await findChangelogSlugs()).map((slug) => `/changelog/${slug}`);
const urls = [...new Set([...routeUrls, ...tutorialUrls, ...changelogUrls])].sort((left, right) =>
  left.localeCompare(right),
);
const sitemap = [
  '<?xml version="1.0" encoding="UTF-8"?>',
  '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
  ...urls.map(toXmlUrl),
  '</urlset>',
  '',
].join('\n');

await fs.writeFile(sitemapPath, sitemap, 'utf8');
console.log(`Generated ${path.relative(repoRoot, sitemapPath)} with ${urls.length} URLs.`);
