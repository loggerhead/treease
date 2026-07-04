import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const docsDir = path.join(repoRoot, 'docs');

const excludedDocDirs = new Set(['archive', 'research']);
const excludedRootFiles = new Set(['README.md', 'README.zh.md', 'AGENTS.md', 'CLAUDE.md']);

process.stdout.on('error', (error) => {
  if (error?.code === 'EPIPE') {
    process.exit(0);
  }
  throw error;
});

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function walkMarkdownFiles(dir, base = dir) {
  const entries = readdirSync(dir, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    if (entry.name.startsWith('.')) continue;

    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (excludedDocDirs.has(entry.name)) continue;
      files.push(...walkMarkdownFiles(fullPath, base));
      continue;
    }

    if (entry.isFile() && entry.name.endsWith('.md')) {
      files.push(normalizePath(path.relative(base, fullPath)));
    }
  }

  return files.sort((a, b) => a.localeCompare(b));
}

function readFrontmatter(fullPath) {
  const content = readFileSync(fullPath, 'utf8');

  if (!content.startsWith('---\n')) {
    return { summary: null, readWhen: [], error: 'missing front matter' };
  }

  const endIndex = content.indexOf('\n---\n', 4);
  if (endIndex === -1) {
    return { summary: null, readWhen: [], error: 'unterminated front matter' };
  }

  const frontmatter = content.slice(4, endIndex);
  const lines = frontmatter.split('\n');
  let summary = null;
  const readWhen = [];
  let inReadWhen = false;

  for (const rawLine of lines) {
    const line = rawLine.trim();

    if (line.startsWith('summary:')) {
      summary = line
        .slice('summary:'.length)
        .trim()
        .replace(/^['"]|['"]$/g, '');
      inReadWhen = false;
      continue;
    }

    if (line.startsWith('read_when:')) {
      inReadWhen = true;
      continue;
    }

    if (inReadWhen && line.startsWith('- ')) {
      readWhen.push(line.slice(2).trim());
      continue;
    }

    if (line !== '') {
      inReadWhen = false;
    }
  }

  if (!summary) {
    return { summary: null, readWhen, error: 'summary key missing' };
  }

  return { summary, readWhen };
}

function printEntry(relativePath, metadata) {
  const { summary, readWhen, error } = metadata;
  if (!summary) {
    console.log(`${relativePath} - [${error ?? 'missing metadata'}]`);
    return;
  }

  console.log(`${relativePath} - ${summary}`);
  if (readWhen.length > 0) {
    console.log(`  Read when: ${readWhen.join('; ')}`);
  }
}

console.log('Docs index for Treease:\n');

for (const relativePath of walkMarkdownFiles(docsDir)) {
  printEntry(relativePath, readFrontmatter(path.join(docsDir, relativePath)));
}

const rootMarkdownFiles = readdirSync(repoRoot, { withFileTypes: true })
  .filter(
    (entry) =>
      entry.isFile() &&
      entry.name.endsWith('.md') &&
      !entry.name.startsWith('.') &&
      !excludedRootFiles.has(entry.name),
  )
  .map((entry) => entry.name)
  .sort((a, b) => a.localeCompare(b));

if (rootMarkdownFiles.length > 0) {
  console.log('\nRoot-level markdown files:\n');
  for (const fileName of rootMarkdownFiles) {
    printEntry(fileName, readFrontmatter(path.join(repoRoot, fileName)));
  }
}

if (!existsSync(docsDir)) {
  console.log('\nWarning: docs/ does not exist.');
}

console.log(
  '\nReminder: run the matching docs in "Read when" before searching, reading code, editing files, running tests, or answering repository-specific questions.',
);
