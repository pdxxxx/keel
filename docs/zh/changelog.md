# 更新日志

## 1.1.0 — 2026-11-24

面向存量老仓库（1→N）与旧框架迁移的版本。

- **增量验证门禁**：`verify.gates` 支持可选 `paths` glob，新增 `keel verify --git / --staged / --base <ref>`（Node 与 Rust MCP 的 `keel_verify` 同步支持 `git: true`）。
- **规则体检**：新增 `keel rules check --stale` / `--strict`（MCP `keel_rules_check` 同步支持 `stale: true`），检测死路径 glob、超过 80 行的规则与超过 5 条的全局规则，默认行为保持 1.0.0 兼容。
- **Git 热点排雷**：新增 `keel hotspots`，按近期改动与 bugfix 提交对现存文件排序。
- **一次性迁移工具**：新增 `keel migrate scan / plan / cleanup / rollback`（共用外部 SHA-256 快照与冲突保护），并由 `keel init` 内置安装 `keel-migrate` skill。
- **自动发版**：新增 `.github/workflows/release.yml`，支持 npm OIDC Trusted Publishing（`@pdxxxx/keel`）与 Linux / macOS / Windows 三端 Rust `keel-mcp` 二进制自动发布。

## 1.0.0 — 2026-11-24

首个正式版本。

- `keel init`：创建 `.keel/`、自动探测校验脚本、幂等写入 `AGENTS.md` 托管块与标准 skills。
- `keel rules`：`list`、`match`（路径 glob 与 `--git` / `--staged` / `--base`）、`add`、`check`。
- `keel task`：`create`、`list`、`show`、`done`、`reopen`、`archive`。
- `keel verify`：按 `.keel/config.yaml` 运行确定性门禁（支持 `--gate`、`--fail-fast`、`--json`）。
- `keel mem`：`sources`、`sessions`、`search`、`show`，跨 `.keel/` 与本地 Claude Code / Codex / OpenCode / 自定义 JSONL 会话检索。
- `keel ablate` / `keel restore`：带 SHA-256 校验与冲突保护的可逆消融与恢复。
- `keel mcp` 与 `crates/keel-mcp`：Node.js 与原生 Rust 两套 stdio MCP 实现，通过 `parity.mjs` 双端对照。
