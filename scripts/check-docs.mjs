import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const governanceDocs = collectGovernanceDocs();

const errors = [];

function fail(message) {
  errors.push(message);
}

function readRepoFile(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), 'utf8');
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
  return [...collectRootMarkdownFiles(), ...collectMarkdownFiles('docs')]
    .filter((path) => !path.startsWith('docs/dev-loop/'));
}

function walkRepo(relativePath, results) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) return;
  const stats = statSync(absolutePath);
  if (stats.isDirectory()) {
    for (const entry of readdirSync(absolutePath)) {
      walkRepo(path.join(relativePath, entry), results);
    }
    return;
  }
  if (/\.(md|svelte|ts|tsx|js|mjs|json|yml|yaml)$/.test(relativePath)) {
    results.push(relativePath);
  }
}

function isLikelyPathToken(token) {
  if (token.includes('://') || token.startsWith('file:///')) return false;
  if (token.includes(' ')) return false;
  if (token.includes('*')) return false;
  if (/^(pnpm|node|git|vp|playwright)\b/.test(token)) return false;
  if (/^[A-Za-z0-9_-]+:$/.test(token)) return false;
  return /^(\.\.\/|\.\/|apps\/|packages\/|doc\/|scripts\/|\.github\/)/.test(token);
}

function resolveDocPath(docPath, token) {
  const [cleanToken] = token.split('#');
  const baseDir = path.dirname(path.join(repoRoot, docPath));
  return cleanToken.startsWith('./') || cleanToken.startsWith('../')
    ? path.resolve(baseDir, cleanToken)
    : path.resolve(repoRoot, cleanToken);
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
  const tokens = [...content.matchAll(/`([^`\n]+)`/g)].map((match) => match[1].trim());
  for (const token of tokens) {
    if (!isLikelyPathToken(token)) continue;
    const resolved = resolveDocPath(docPath, token);
    if (!existsSync(resolved)) {
      collector(`${docPath}: 路径不存在 -> ${token}`);
      continue;
    }
    validateLineFragment(docPath, token, resolved, collector);
  }
}

function validateCommands(docPath, content, webScripts) {
  const lines = content.split('\n');
  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;

    if (line.startsWith('pnpm ')) {
      const command = line.split(/\s+/)[1];
      if (!command || ['install', 'exec', 'dlx'].includes(command)) continue;
      if (!webScripts[command]) {
        fail(`${docPath}: 未在 apps/web/package.json 中找到脚本 -> pnpm ${command}`);
      }
      continue;
    }

    if (line.startsWith('node ')) {
      const scriptPath = line.slice('node '.length).trim().split(/\s+/)[0];
      const resolved = path.resolve(repoRoot, scriptPath);
      if (!existsSync(resolved)) {
        fail(`${docPath}: Node 脚本不存在 -> ${scriptPath}`);
      }
      continue;
    }
  }
}

function validateAgentRoutingContract() {
  const compactGuide = path.join(repoRoot, 'docs/agent-entrypoints.md');
  if (!existsSync(compactGuide)) {
    fail('docs/agent-entrypoints.md: 缺少 agent 最短路径文档');
  }

  const rootAgents = readRepoFile('AGENTS.md');
  const docsIndex = readRepoFile('docs/README.md');

  if (!rootAgents.includes('docs/agent-entrypoints.md')) {
    fail('AGENTS.md: 根导航未指向 docs/agent-entrypoints.md');
  }

  if (!docsIndex.includes('agent-entrypoints.md')) {
    fail('docs/README.md: 文档索引未暴露 agent 最短路径');
  }

  if (rootAgents.includes('README.md` → `CONTEXT.md` → `ARCHITECTURE.md` → `docs/README.md`')) {
    fail('AGENTS.md: 仍存在无条件多跳阅读链');
  }
}

function validateHotDocBudgets() {
  const budgets = {
    'AGENTS.md': 60,
    'docs/README.md': 45,
    'docs/FRONTEND.md': 80,
    'docs/CORE.md': 50,
    'apps/web/AGENTS.md': 24,
    'apps/web/src/AGENTS.md': 22,
    'apps/web/test/AGENTS.md': 20,
    'packages/core/AGENTS.md': 22,
    'packages/core/src/AGENTS.md': 20,
    'packages/core/tests/AGENTS.md': 18,
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
  const webPackage = JSON.parse(readRepoFile('apps/web/package.json'));
  const webScripts = webPackage.scripts ?? {};

  for (const docPath of governanceDocs) {
    const content = readRepoFile(docPath);
    validateDocPaths(docPath, content);
    validateCommands(docPath, content, webScripts);
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
