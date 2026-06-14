# apps/cli 导航

## 作用域
- 本目录负责 CLI acceptance 测试与相关文档约束；默认 Rust CLI 入口在 `../../packages/core/src/tools/treease.rs`。

## 硬约束
- 不在本目录复制 Core 解析、格式化、算子或评估实现。
- 用户可见 CLI 行为改动必须同时考虑 stdout、stderr、exit code 与文件写回。
- 文档、命令、测试入口必须与 `../../packages/core/Cargo.toml` 和 `tests/acceptance/run.sh` 保持一致。

## 验证
- CLI 逻辑：在 `../../packages/core/` 运行 `cargo nextest run --locked --lib cli::`
- CLI acceptance：在当前目录运行 `bash tests/acceptance/run.sh`
