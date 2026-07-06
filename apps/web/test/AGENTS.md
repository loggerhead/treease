---
summary: "apps/web 集成测试与 E2E 的边界、测试策略与文档入口。"
read_when:
  - 需要确认 Web 测试策略与最小复现路径
---

# apps/web/test 导航

## 作用域
- 本目录承载 Web 集成测试、E2E 测试与测试辅助工具。

## 最短路径
- 先读：`../../../docs/index.md`
- 测试规则：`../../../docs/testing/index.md`
- Web 规则：`../../../docs/web/index.md`

## 本地规则
- 集成测试放在 `integration/**/*.test.ts`。
- E2E 测试放在 `e2e/`。
- 优先覆盖真实 Worker/WASM 链路，避免过度 mock。

## 验证
- 按 `../../../docs/testing/index.md` 选择最小相关命令；不要无差别全跑。
