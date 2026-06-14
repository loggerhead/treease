# apps/cli 导航

## 作用域
- 本目录负责独立 CLI crate、CLI acceptance 测试与相关文档约束。
- Rust CLI 入口在 `src/main.rs`，主要实现位于 `src/lib.rs` 与 `src/{parser,spec,catalog,errors}.rs`。

## 硬约束
- 不在本目录复制 Core 解析、格式化、算子或评估实现。
- CLI 通过 `treease-core` 复用执行、格式和算子能力；不要把 CLI 解析/帮助/错误协议再塞回 `packages/core/`。
- 用户可见 CLI 行为改动必须同时考虑 stdout、stderr、exit code 与文件写回。
- 文档、命令、测试入口必须与 `Cargo.toml` 和 `tests/acceptance/run.sh` 保持一致。

## 验证
- CLI 逻辑：在当前目录运行 `cargo nextest run --locked --lib`
- CLI acceptance：在当前目录运行 `bash tests/acceptance/run.sh`
