# Native keel-mcp stdio server

## Goal

- 在 `crates/keel-mcp/` 交付独立的原生 Rust stdio server，复用 Keel CLI 1.0.0 的磁盘格式、九个工具的 schema 和返回契约。

## Non-Goals

- 不改现有 TypeScript、包配置或根目录 CI；不引入 Agent 调度、Hook、网络服务或后台索引。

## Acceptance Criteria

- 用子进程测试握手、通知、九个工具、错误恢复、输出隔离，并与现有 Node 实现做对照。
- 测试 POSIX 路径、Git 变更、任务文件互读、门禁与 JSONL 损坏行、排序及稳定 ID。
- 从 crate 目录实际通过 `cargo test`、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`，并运行仓库门禁。
- README 提供可用的 `mcpServers` 配置、运行边界及验证方法。

## Decisions & Notes

- 明确的协议差异：握手名称为 `keel-mcp`。测试使用临时 HOME，不读取用户会话。
- 检索时逐行处理会话，只保留有限个最佳结果，不创建持久索引。

## 验收记录（2026-11-24）

- Windows x86_64：`cargo fmt --check`、`cargo test`（11 个单元测试 + 7 个 stdio 子进程测试）、`cargo clippy --all-targets -- -D warnings` 和 `cargo build --release` 全部通过。
- `node tests/parity.mjs` 使用 release 二进制：36 项响应对照通过；Rust 与 Node 的任务文件可双向互读。包含 UTF-16 边界、毫秒时间戳与同分排序回归场景。
- 原仓库 `node bin/keel.js verify --json`：typecheck、build、rules-check、test 四项均通过。Linux GNU x86_64 的 `cargo check --all-targets --locked` 通过；未在 Linux/macOS 实机运行。
- 本机合成数据：96 个文件，98,304 个会话轮次，18,229,952 字节。新进程、热文件缓存，排除一次预热，交替运行 Rust 与 Node。握手含退出的耗时中位数（9 次）分别为 3.2 / 58.9 ms；检索前 15 项（5 次）分别为 43.2 / 237.8 ms，返回内容相同。仅为本机样本，不代表冷磁盘或其他平台。
