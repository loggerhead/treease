import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const webDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const origin = 'https://treease.com';
const sitemapPath = path.join(webDir, 'static', 'sitemap.xml');
const tutorialSourcePath = path.join(webDir, 'src/lib/content/tutorials/index.ts');
const changelogSourcePath = path.join(webDir, 'src/lib/content/changelog.generated.ts');

const escapeXml = (value) => value
  .replaceAll('&', '&amp;')
  .replaceAll('<', '&lt;')
  .replaceAll('>', '&gt;')
  .replaceAll('"', '&quot;')
  .replaceAll("'", '&apos;');

function extractSlugs(source, pattern, label) {
  const slugs = [...source.matchAll(pattern)].map((match) => match[1]);
  if (slugs.length === 0) throw new Error(`No ${label} slugs found`);
  if (new Set(slugs).size !== slugs.length) throw new Error(`Duplicate ${label} slug found`);
  return slugs;
}

const [tutorialSource, changelogSource] = await Promise.all([
  readFile(tutorialSourcePath, 'utf8'),
  readFile(changelogSourcePath, 'utf8'),
]);
const tutorialSlugs = extractSlugs(tutorialSource, /\bslug:\s*'([^']+)'/g, 'tutorial');
const changelogSlugs = extractSlugs(changelogSource, /"slug":\s*"([^"]+)"/g, 'changelog');
const paths = [
  '/',
  '/about',
  '/changelog',
  ...changelogSlugs.map((slug) => `/changelog/${slug}`),
  '/privacy',
  '/terms',
  '/tutorial',
  ...tutorialSlugs.map((slug) => `/tutorial/${slug}`),
];

const uniquePaths = [...new Set(paths)];
const body = uniquePaths.map((pagePath) => `  <url><loc>${escapeXml(`${origin}${pagePath}`)}</loc></url>`).join('\n');
await writeFile(sitemapPath, `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${body}\n</urlset>\n`);
console.log(`Generated ${uniquePaths.length} Sitemap URLs from tutorial and Changelog content.`);
