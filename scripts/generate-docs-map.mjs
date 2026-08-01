#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, writeFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = process.cwd();
const DOCS_DIR = join(ROOT, 'docs');
const OUTPUT_PATH = join(DOCS_DIR, 'docs_map.md');
const MARKDOWN_EXTENSIONS = /\.mdx?$/i;
const EXCLUDED_DIRS = new Set([
  '.generated',
  'adr',
  'archive',
  'assets',
  'images',
  'internal',
  'operators',
  'research',
  'snippets',
  'template',
]);
const EXCLUDED_FILES = new Set(['AGENTS.md', 'CLAUDE.md', 'docs_map.md']);

if (!existsSync(DOCS_DIR)) {
  console.error('docs:map: missing docs directory. Run from repo root.');
  process.exit(1);
}

if (!statSync(DOCS_DIR).isDirectory()) {
  console.error('docs:map: docs path is not a directory.');
  process.exit(1);
}

function normalizeSlashes(value) {
  return value.split('\\').join('/');
}

function stripFrontmatter(raw) {
  if (!raw.startsWith('---\n') && !raw.startsWith('---\r\n')) {
    return raw;
  }

  const lines = raw.split(/\r?\n/u);
  for (let index = 1; index < lines.length; index += 1) {
    if (lines[index] === '---' || lines[index] === '...') {
      return lines.slice(index + 1).join('\n');
    }
  }

  return raw;
}

function escapeMarkdownHtmlText(value) {
  return value.replace(/&/gu, '&amp;').replace(/</gu, '&lt;').replace(/>/gu, '&gt;');
}

function cleanHeadingText(value) {
  const normalized = value
    .replace(/\s+#+\s*$/u, '')
    .replace(/\[[^\]]+\]\([^)]*\)/gu, (match) => match.replace(/^\[/u, '').replace(/\][^(]*\([^)]*\)$/u, ''))
    .replace(/[*_~`]/gu, '')
    .replace(/\s+/gu, ' ')
    .trim();

  return escapeMarkdownHtmlText(normalized);
}

function extractHeadings(raw) {
  const headings = [];
  const lines = stripFrontmatter(raw).split(/\r?\n/u);
  let fenceMarker = null;

  for (const rawLine of lines) {
    const trimmed = rawLine.trim();
    const fenceMatch = /^(?<marker>`{3,}|~{3,})/u.exec(trimmed);
    if (fenceMatch) {
      const marker = fenceMatch.groups.marker[0];
      fenceMarker = fenceMarker === marker ? null : (fenceMarker ?? marker);
      continue;
    }
    if (fenceMarker) {
      continue;
    }

    const match = /^(#{1,4})\s+(.+)$/u.exec(rawLine);
    if (!match) {
      continue;
    }

    const text = cleanHeadingText(match[2]);
    if (text) {
      headings.push({ depth: match[1].length, text });
    }
  }

  return headings;
}

function routeForFile(relativePath) {
  const withoutExtension = relativePath.replace(/\.mdx?$/iu, '');

  if (withoutExtension === 'index') {
    return '/';
  }

  if (withoutExtension.endsWith('/index')) {
    return `/${withoutExtension.slice(0, -'/index'.length)}`;
  }

  return `/${withoutExtension}`;
}

function walkMarkdownFiles(dir, base = dir) {
  const entries = readdirSync(dir, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    if (entry.name.startsWith('.')) {
      continue;
    }

    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (EXCLUDED_DIRS.has(entry.name)) {
        continue;
      }
      files.push(...walkMarkdownFiles(fullPath, base));
      continue;
    }

    if (!entry.isFile() || !MARKDOWN_EXTENSIONS.test(entry.name)) {
      continue;
    }

    const rel = normalizeSlashes(relative(base, fullPath));
    const baseName = rel.split('/').at(-1);
    if (EXCLUDED_FILES.has(baseName)) {
      continue;
    }
    files.push(rel);
  }

  return files.toSorted((left, right) => (left < right ? -1 : left > right ? 1 : 0));
}

function buildMapEntries() {
  const lines = [];
  const headingLine = [
    '# Treease docs map',
    '',
    'This file is generated from `docs/**/*.md` and `docs/**/*.mdx` headings, excluding the private `docs/template/` conversation templates, to help agents navigate the documentation tree.',
    'Do not edit it by hand; run `pnpm docs:map:gen`.',
    '',
  ];

  for (const relativePath of walkMarkdownFiles(DOCS_DIR)) {
    const absolutePath = join(DOCS_DIR, relativePath);
    const headings = extractHeadings(readFileSync(absolutePath, 'utf8'));

    lines.push(`## ${relativePath}`);
    lines.push('');
    lines.push(`- Route: ${routeForFile(relativePath)}`);
    lines.push('- Headings:');

    if (headings.length === 0) {
      lines.push('  - No headings found');
    } else {
      for (const heading of headings) {
        lines.push(`  - H${heading.depth}: ${heading.text}`);
      }
    }

    lines.push('');
  }

  return headingLine.concat(lines);
}

const expected = buildMapEntries();
const expectedContent = `${expected.join('\n').trimEnd()}\n`;

if (process.argv.includes('--check')) {
  if (!existsSync(OUTPUT_PATH)) {
    console.error('docs:map: docs/docs_map.md is missing. Run `pnpm docs:map:gen`.');
    process.exit(1);
  }

  const currentContent = readFileSync(OUTPUT_PATH, 'utf8');
  if (currentContent !== expectedContent) {
    console.error('docs:map: docs/docs_map.md is out of date. Run `pnpm docs:map:gen`.');
    process.exit(1);
  }

  console.log('docs:map: docs/docs_map.md is up to date.');
  process.exit(0);
}

writeFileSync(OUTPUT_PATH, expectedContent, 'utf8');
console.log(`docs:map: updated ${OUTPUT_PATH}`);
