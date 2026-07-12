# apps/desktop

本目录承载 `Desktop Workspace` 的 Tauri 宿主、平台 capability、打包和桌面端测试。

- 开始之前阅读 `../../docs/desktop/index.md` 与 `../../docs/desktop/implementation.md`。
- 共享 UI 和文档主链仍留在 `../web/` 和 `../../packages/core/`；不在这里复制解析、格式化、图构建或 snapshot 语义。
- 一切平台能力经由 `Workspace Host` 契约；不让共享组件直接调用 Tauri API。
- 只授权用户明确交付的文件；不增加目录扫描或宽范围文件系统权限。
- 不在 `packages/core` 引入桌面相关依赖或条件逻辑。
