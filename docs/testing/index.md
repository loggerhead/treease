---
summary: "Testing 主题域入口，承接测试分层、验证命令与真实覆盖原则。"
read_when:
  - 需要决定该跑什么测试或补哪一层测试
  - 需要核对最小相关验证策略
---
# Testing

`testing/` 收编 Treease 的测试策略、回归约束与验证命令。这里先回答“该在哪一层测”，再回答“该跑哪个命令”。

## Core Principles

- 测试覆盖真实用户依赖的主链，不追求伪覆盖。
- 先选最小相关验证，再决定是否扩大范围。
- 文档、协议、Worker/WASM、GraphViewer 等跨边界行为优先验证对外可观察结果。

## Layer Selection

- Core 语义、operator、format、protocol 回归：优先 `packages/core` 的 Rust 测试。
- CLI 命令行为、错误码、导出快照：优先 `apps/cli` 的 lib 和 acceptance 测试。
- Web 内部状态、controller、render helper：优先 `apps/web` 单测。
- Worker / WASM / GraphViewer / Workspace 跨边界链路：优先 `apps/web` 集成测试。
- 需要验证真实页面行为、图形交互或端到端体验时：再上 E2E。

## Verification Commands

- Core：`cd packages/core && cargo nextest run --locked`
- CLI：`cd apps/cli && cargo nextest run --locked --lib`
- CLI acceptance：`cd apps/cli && bash tests/acceptance/run.sh`
- Web unit：`cd apps/web && pnpm test:unit`
- Web integration：`cd apps/web && pnpm test:integration`
- Web E2E：`cd apps/web && pnpm test:e2e`
- Server 类型检查：`cd apps/server && ./node_modules/.bin/tsc -p tsconfig.json --noEmit`
- Server tests：`cd apps/server && node --import tsx --test src/**/*.test.ts`
- Docs 结构校验：`node scripts/check-docs.mjs`

## What Requires Real-Chain Proof

- protocol 或 WASM 生成链路改动
- snapshot-bound read、mainGraph、planner 或 graph edit 主链改动
- Worker / Web 的 freshness、过期结果丢弃、异步落地改动
- GraphViewer、workspace、streaming 或 layout 的用户可见行为改动
- CLI 对外命令、错误码、机器可读产物改动

## Related Domains

- Web 交互与前端 runtime：`../web/index.md`
- Core 语义与协议真源：`../core/index.md`
