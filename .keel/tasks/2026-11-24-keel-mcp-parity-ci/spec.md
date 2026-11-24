# Align Keel rule delimiters, task IDs and native CI

## Goal

- 同步 Keel CLI 和原生 MCP 1.0.0 的规则分隔符与任务 ID 契约，并把双端对照接入现有 CI。

## Non-Goals

- 不改版本、MCP 工具清单或宿主调度，不提交、不推送。

## Acceptance Criteria

- TS / Rust 单测和 stdio 对照均确认：仅整行 `---` 结束 frontmatter，顶格键与缩进块文本不丢失。
- 在文件访问前拒绝含父目录分段或绝对路径的任务 ID；已有活动、归档任务保持可读，非法 ID 不会修改状态或移动任务。
- Ubuntu / Windows CI 构建 TS 与 Rust 并执行对照；根 README 说明原生 MCP 的构建和接入方式。
- 运行根 `node bin/keel.js verify` 和 crate 的 `cargo test`、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`、`node tests/parity.mjs`。

## Decisions & Notes

- 两种路径分隔符均按任务目录分段处理；普通名字里的连续点不是父目录分段。对照测试除比较两端相等，还断言预期结果。

## 验收记录（2026-11-24）

- Windows：根 `node bin/keel.js verify` 四项全部通过。Rust `cargo test`（12 个单元、7 个 stdio 测试）、Clippy、release 构建均通过，CI 的 `--locked` 命令也通过。
- 从根目录与 crate 目录均实际运行了 release 对照脚本；49 项响应及双向任务文件互读通过。另以 Windows Node 20.20.2、22.23.5 分别复验通过。
- 已检查 CI 矩阵、步骤顺序、README 配置和文档链接。Linux GNU x86_64 跨目标 `cargo check --all-targets --locked` 通过；未触发远程 CI，不视为 Ubuntu 实机测试。
