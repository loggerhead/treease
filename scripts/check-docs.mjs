import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const governanceDocs = collectGovernanceDocs();
const cargoBinsByDir = collectCargoBinNamesByDir();
const cargoBins = new Set([...cargoBinsByDir.values()].flatMap((bins) => [...bins]));
const packageScriptsByDir = collectPackageScriptsByDir();

const errors = [];

function fail(message) {
  errors.push(message);
}

function readRepoFile(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

function parseFrontmatter(content) {
  if (!content.startsWith('---\n')) {
    return { error: 'missing front matter' };
  }

  const endIndex = content.indexOf('\n---\n', 4);
  if (endIndex === -1) {
    return { error: 'unterminated front matter' };
  }

  const frontmatter = content.slice(4, endIndex);
  const summaryMatch = frontmatter.match(/^summary:\s*(.+)$/m);
  if (!summaryMatch) {
    return { error: 'summary key missing' };
  }

  const summary = summaryMatch[1].trim().replace(/^['"]|['"]$/g, '');
  if (!summary) {
    return { error: 'summary is empty' };
  }

  const readWhen = [];
  let inReadWhen = false;
  for (const rawLine of frontmatter.split('\n')) {
    const line = rawLine.trim();
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

  return { summary, readWhen };
}

function requiresDocMetadata(docPath) {
  if (['ARCHITECTURE.md', 'CONTEXT.md', 'guess-failure.md'].includes(docPath)) {
    return true;
  }

  if (!docPath.startsWith('docs/')) {
    return false;
  }

  if (docPath.startsWith('docs/operators/')) {
    return docPath === 'docs/operators/README.md';
  }

  if (docPath.startsWith('docs/generated/')) {
    return true;
  }

  if (docPath.startsWith('docs/references/')) {
    return docPath === 'docs/references/README.md' || docPath === 'docs/references/yaml-common-subset.md';
  }

  if (docPath.startsWith('docs/formats/')) {
    return docPath === 'docs/formats/README.md' || docPath.endsWith('.md');
  }

  return true;
}

function normalizeRelativePath(relativePath) {
  return relativePath.split(path.sep).join('/');
}

function collectMarkdownFiles(relativePath) {
  const results = [];
  walkRepo(relativePath, results);
  return results
    .filter((item) => item.endsWith('.md'))
    .map((item) => normalizeRelativePath(item))
    .sort();
}

function collectRootMarkdownFiles() {
  return readdirSync(repoRoot)
    .filter((entry) => entry.endsWith('.md'))
    .sort();
}

function collectGovernanceDocs() {
  return [
    ...new Set([
      ...collectRootMarkdownFiles(),
      ...collectMarkdownFiles('docs'),
      ...collectNamedFiles('apps', 'AGENTS.md'),
      ...collectNamedFiles('packages', 'AGENTS.md'),
    ]),
  ]
    .filter(
      (path) => !path.startsWith('docs/dev-loop/'),
    );
}

function collectNamedFiles(relativePath, targetName) {
  const results = [];
  walkRepo(relativePath, results, { matchAllFiles: true });
  return results
    .filter((item) => path.basename(item) === targetName)
    .map((item) => normalizeRelativePath(item))
    .sort();
}

function walkRepo(relativePath, results, options = {}) {
  const { matchAllFiles = false } = options;
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) return;
  const stats = statSync(absolutePath);
  if (stats.isDirectory()) {
    if (shouldSkipDir(relativePath)) return;
    for (const entry of readdirSync(absolutePath)) {
      walkRepo(path.join(relativePath, entry), results, options);
    }
    return;
  }
  if (matchAllFiles || /\.(md|svelte|ts|tsx|js|mjs|json|yml|yaml)$/.test(relativePath)) {
    results.push(relativePath);
  }
}

function shouldSkipDir(relativePath) {
  const base = path.basename(relativePath);
  return [
    '.git',
    '.tmp',
    '.venv',
    '.venv-playwright',
    'node_modules',
  ].includes(base);
}

function isPlaceholderToken(token) {
  return /[<>{}]/.test(token);
}

function isLikelyPathToken(token) {
  if (token.includes('://') || token.startsWith('file:///')) return false;
  if (token.includes(' ')) return false;
  if (token.includes('*')) return false;
  if (isPlaceholderToken(token)) return false;
  if (/^(pnpm|node|git|vp|playwright)\b/.test(token)) return false;
  if (/^[A-Za-z0-9_-]+:$/.test(token)) return false;
  return /^(\.\.\/|\.\/|apps\/|packages\/|docs\/|test\/|scripts\/|\.github\/)/.test(token);
}

function isLikelyMarkdownLinkTarget(token) {
  if (!token || token.startsWith('#')) return false;
  if (/^(https?:|mailto:|file:)/.test(token)) return false;
  if (isPlaceholderToken(token)) return false;
  return true;
}

function collectInlineCodeTokens(content) {
  return [...content.matchAll(/`([^`\n]+)`/g)].map((match) => match[1].trim());
}

function collectMarkdownLinkTargets(content) {
  return [...content.matchAll(/\[[^\]]+\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)].map((match) =>
    match[1].trim(),
  );
}

function collectCargoBinNamesByDir() {
  const manifests = [
    ...collectNamedFiles('apps', 'Cargo.toml'),
    ...collectNamedFiles('packages', 'Cargo.toml'),
  ];
  const binsByDir = new Map();
  for (const manifestPath of manifests) {
    const content = readRepoFile(manifestPath);
    const dir = normalizeRelativePath(path.dirname(manifestPath));
    const bins = new Set();
    let inBin = false;
    for (const rawLine of content.split('\n')) {
      const line = rawLine.trim();
      if (line === '[[bin]]') {
        inBin = true;
        continue;
      }
      if (line.startsWith('[')) {
        inBin = false;
      }
      if (!inBin) continue;
      const match = line.match(/^name\s*=\s*"([^"]+)"/);
      if (match) bins.add(match[1]);
    }
    binsByDir.set(dir, bins);
  }
  return binsByDir;
}

function collectPackageScriptsByDir() {
  const manifests = [
    'package.json',
    ...collectNamedFiles('apps', 'package.json'),
    ...collectNamedFiles('packages', 'package.json'),
  ];
  const scriptsByDir = new Map();
  for (const manifestPath of manifests) {
    const content = readRepoFile(manifestPath);
    const dir = normalizeRelativePath(path.dirname(manifestPath));
    const packageJson = JSON.parse(content);
    scriptsByDir.set(dir === '.' ? '' : dir, packageJson.scripts ?? {});
  }
  return scriptsByDir;
}

function hasScriptInAnyPackage(command) {
  for (const scripts of packageScriptsByDir.values()) {
    if (scripts[command]) return true;
  }
  return false;
}

function resolveInlineDocPath(docPath, token) {
  const [cleanToken] = token.split('#');
  const baseDir = path.dirname(path.join(repoRoot, docPath));
  return cleanToken.startsWith('./') || cleanToken.startsWith('../')
    ? path.resolve(baseDir, cleanToken)
    : path.resolve(repoRoot, cleanToken);
}

function resolveMarkdownLinkTarget(docPath, token) {
  const [cleanToken] = token.split('#');
  const baseDir = path.dirname(path.join(repoRoot, docPath));
  if (/^(apps\/|packages\/|docs\/|test\/|scripts\/|\.github\/)/.test(cleanToken)) {
    return path.resolve(repoRoot, cleanToken);
  }
  return path.resolve(baseDir, cleanToken);
}

function validateLineFragment(docPath, token, resolved, collector) {
  const fragment = token.split('#')[1];
  if (!fragment) return;
  const match = fragment.match(/^L(\d+)(?:-L(\d+))?$/);
  if (!match) {
    collector(`${docPath}: 未识别的行号片段 -> ${token}`);
    return;
  }
  const start = Number(match[1]);
  const end = Number(match[2] ?? match[1]);
  const content = readFileSync(resolved, 'utf8');
  const lineCount = content.split('\n').length;
  if (start < 1 || end < start || end > lineCount) {
    collector(`${docPath}: 行号片段越界 -> ${token}`);
  }
}

function validateDocPaths(docPath, content, collector = fail) {
  const inlineTokens = new Set(collectInlineCodeTokens(content).filter(isLikelyPathToken));
  for (const token of inlineTokens) {
    const resolved = resolveInlineDocPath(docPath, token);
    if (!existsSync(resolved)) {
      collector(`${docPath}: 路径不存在 -> ${token}`);
      continue;
    }
    validateLineFragment(docPath, token, resolved, collector);
  }

  const markdownTargets = new Set(
    collectMarkdownLinkTargets(content).filter(isLikelyMarkdownLinkTarget),
  );
  for (const token of markdownTargets) {
    const resolved = resolveMarkdownLinkTarget(docPath, token);
    if (!existsSync(resolved)) {
      collector(`${docPath}: 路径不存在 -> ${token}`);
      continue;
    }
    validateLineFragment(docPath, token, resolved, collector);
  }
}

function validateCommands(docPath, content) {
  for (const token of collectInlineCodeTokens(content)) {
    validateCommandToken(docPath, token);
  }
}

function resolveCommandPath(cwd, targetPath) {
  return path.resolve(cwd, targetPath);
}

function findCargoManifestDir(cwd) {
  let currentDir = cwd;
  while (currentDir.startsWith(repoRoot)) {
    if (existsSync(path.join(currentDir, 'Cargo.toml'))) return currentDir;
    if (currentDir === repoRoot) break;
    currentDir = path.dirname(currentDir);
  }
  return null;
}

function findNearestPackageDir(cwd) {
  let currentDir = cwd;
  while (currentDir.startsWith(repoRoot)) {
    if (existsSync(path.join(currentDir, 'package.json'))) return currentDir;
    if (currentDir === repoRoot) break;
    currentDir = path.dirname(currentDir);
  }
  return null;
}

function validateCommandToken(docPath, token) {
  let cwd = repoRoot;
  for (const rawSegment of token.split('&&')) {
    const segment = rawSegment.trim();
    if (!segment) continue;

    if (segment.startsWith('cd ')) {
      const targetPath = segment.slice('cd '.length).trim().split(/\s+/)[0];
      const resolved = resolveCommandPath(cwd, targetPath);
      if (!existsSync(resolved)) {
        fail(`${docPath}: cd 目标不存在 -> ${targetPath}`);
        continue;
      }
      cwd = resolved;
      continue;
    }

    if (segment.startsWith('pnpm ')) {
      const parts = segment.split(/\s+/);
      const command = parts[1];
      if (!command || ['install', 'exec', 'dlx'].includes(command)) continue;

      if (command === '--dir') {
        const dir = parts[2];
        const nestedCommand = parts[3];
        const normalizedDir = normalizeRelativePath(dir);
        const packageScripts = packageScriptsByDir.get(normalizedDir);
        if (nestedCommand && packageScripts && !packageScripts[nestedCommand]) {
          fail(`${docPath}: 未在 ${normalizedDir}/package.json 中找到脚本 -> pnpm ${nestedCommand}`);
        }
        continue;
      }

      const packageDir = findNearestPackageDir(cwd);
      const normalizedDir = packageDir
        ? normalizeRelativePath(path.relative(repoRoot, packageDir))
        : '';
      const packageScripts = packageScriptsByDir.get(normalizedDir);
      if (packageScripts && !packageScripts[command] && !hasScriptInAnyPackage(command)) {
        const packageLabel = normalizedDir ? `${normalizedDir}/package.json` : '根 package.json';
        fail(`${docPath}: 未在 ${packageLabel} 中找到脚本 -> pnpm ${command}`);
      }
      continue;
    }

    if (segment.startsWith('node ')) {
      const scriptPath = segment.slice('node '.length).trim().split(/\s+/)[0];
      if (!scriptPath || scriptPath.startsWith('-')) continue;
      const resolved = resolveCommandPath(cwd, scriptPath);
      if (!existsSync(resolved)) {
        fail(`${docPath}: Node 脚本不存在 -> ${scriptPath}`);
      }
      continue;
    }

    if (segment.startsWith('bash ')) {
      const scriptPath = segment.slice('bash '.length).trim().split(/\s+/)[0];
      const resolved = resolveCommandPath(cwd, scriptPath);
      if (!existsSync(resolved)) {
        fail(`${docPath}: Bash 脚本不存在 -> ${scriptPath}`);
      }
      continue;
    }

    if (segment.startsWith('cargo run ')) {
      const match = segment.match(/--bin\s+([A-Za-z0-9_-]+)/);
      if (!match) continue;
      const manifestDir = findCargoManifestDir(cwd);
      const knownBins = manifestDir
        ? cargoBinsByDir.get(normalizeRelativePath(path.relative(repoRoot, manifestDir))) ??
          cargoBins
        : cargoBins;
      if (!knownBins.has(match[1])) {
        fail(`${docPath}: Cargo bin 不存在 -> ${match[1]}`);
      }
    }
  }
}

function validateAgentRoutingContract() {
  const compactGuide = path.join(repoRoot, 'docs/agent-entrypoints.md');
  if (!existsSync(compactGuide)) {
    fail('docs/agent-entrypoints.md: 缺少 agent 最短路径文档');
  }

  const rootAgents = readRepoFile('AGENTS.md');
  if (!rootAgents.includes('docs/agent-entrypoints.md')) {
    fail('AGENTS.md: 根导航未指向 docs/agent-entrypoints.md');
  }

  if (rootAgents.includes('README.md` → `CONTEXT.md` → `ARCHITECTURE.md` → `docs/README.md`')) {
    fail('AGENTS.md: 仍存在无条件多跳阅读链');
  }

  if (!rootAgents.includes('pnpm docs:list')) {
    fail('AGENTS.md: 缺少 pnpm docs:list 前置约束');
  }
}

function validateDocMetadata(docPath, content) {
  if (!requiresDocMetadata(docPath)) return;

  const { summary, error } = parseFrontmatter(content);
  if (!summary) {
    fail(`${docPath}: 文档元信息缺失 -> ${error}`);
  }
}

function validateHotDocBudgets() {
  const budgets = {
    'AGENTS.md': 65,
    'docs/agent-entrypoints.md': 60,
    'docs/FRONTEND.md': 160,
    'docs/CORE.md': 115,
    'apps/web/AGENTS.md': 24,
    'apps/web/test/AGENTS.md': 20,
    'packages/core/AGENTS.md': 22,
    'apps/cli/AGENTS.md': 24,
  };

  for (const [relativePath, maxLines] of Object.entries(budgets)) {
    const lineCount = readRepoFile(relativePath).split('\n').length;
    if (lineCount > maxLines) {
      fail(`${relativePath}: 行数 ${lineCount} 超过热路径预算 ${maxLines}`);
    }
  }
}

function main() {
  for (const docPath of governanceDocs) {
    const content = readRepoFile(docPath);
    validateDocMetadata(docPath, content);
    validateDocPaths(docPath, content);
    validateCommands(docPath, content);
  }

  validateAgentRoutingContract();
  validateHotDocBudgets();

  if (errors.length > 0) {
    console.error('文档一致性校验失败：');
    for (const error of errors) {
      console.error(`- ${error}`);
    }
    process.exit(1);
  }

  console.log(`文档一致性校验通过，共检查 ${governanceDocs.length} 个治理文档。`);
}

main();
