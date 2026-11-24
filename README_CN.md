<div align="center">

<img src="./assets/keel.png" width="84" height="84" alt="keel" />

# keel

**面向 AI 编程的仓库规范、任务记录、会话检索与验证门禁工具。**

<p>
  <a href="https://github.com/pdxxxx/keel/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/pdxxxx/keel/ci.yml?style=flat-square&label=CI" alt="CI" /></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-2563eb?style=flat-square" alt="MIT License" /></a>
  <img src="https://img.shields.io/badge/node-%3E%3D20-339933?style=flat-square" alt="Node >= 20" />
  <img src="https://img.shields.io/badge/rust-1.85%2B-dea584?style=flat-square" alt="Rust 1.85+" />
</p>

<p>
  <a href="./README.md">English</a> ·
  <a href="./ARCHITECTURE.md">Architecture</a> ·
  <a href="./crates/keel-mcp/README.md">Rust MCP</a> ·
  <a href="./CHANGELOG.md">Changelog</a>
</p>

</div>

---

Keel 把项目规则、可选任务文档和校验命令以纯文本形式存在 `.keel/` 下，并支持跨 Claude Code、Codex、OpenCode 与自定义目录检索本地历史会话。

它不接管子 Agent 调度，不往编辑器里装私有 Hook，也不绑定特定 AI 工具。直接用命令行，或者通过 stdio 接 MCP 都可以。

## 快速开始

需要 **Node.js >= 20**。

```sh
pnpm install && pnpm build
```

```sh
# 1. 初始化 .keel/、AGENTS.md 托管块与标准 .agents/skills/
keel init

# 2. 添加一条按路径生效的规则
keel rules add auth-rotation \
  --summary "Rotate refresh tokens atomically" \
  --paths "src/auth/**"

# 3. 查找当前 git diff 命中的规则
keel rules match --git

# 4. 检索历史会话和 .keel/ 文档
keel mem search "refresh token"

# 5. 运行 .keel/config.yaml 里配置的校验命令
keel verify
```

## 命令一览

所有命令均支持 `--json`。

| 命令 | 说明 |
| --- | --- |
| `keel init` | 创建 `.keel/`、自动识别常见检查脚本（`typecheck` / `lint` / `test`）、更新 `AGENTS.md` 并写入 `.agents/skills/`。 |
| `keel rules` | `list`、`match [paths...] [--git \| --staged \| --base <ref>]`、`add <id>`、`check`。 |
| `keel task` | `create`、`list`、`show`、`done`、`reopen`、`archive`，按需用单个 `spec.md` 记录复杂任务。 |
| `keel verify` | 按顺序运行 `.keel/config.yaml` 中的检查命令（支持 `--gate`、`--fail-fast`）。 |
| `keel mem` | `sources`、`sessions`、`search <query>`、`show <id>`，检索本地 AI 会话与 `.keel/` 文档。 |
| `keel ablate` / `restore` | 将 Keel 相关文件临时移入带 SHA-256 校验的外部快照以做对比测试，随后无损恢复。 |
| `keel mcp` | 启动内置 stdio MCP 服务。 |

## MCP 接入

仓库同时提供两套协议完全一致的 stdio MCP 实现：Node.js 版（`keel mcp`）和独立 Rust 二进制版（`crates/keel-mcp`，约 2 MiB，运行不依赖 Node）。

**Node.js 版**

```json
{
  "mcpServers": {
    "keel": {
      "command": "keel",
      "args": ["mcp"]
    }
  }
}
```

**原生 Rust 版**（`cargo build --release --locked --manifest-path crates/keel-mcp/Cargo.toml`）

```json
{
  "mcpServers": {
    "keel": {
      "command": "/path/to/keel/crates/keel-mcp/target/release/keel-mcp",
      "args": ["--root", "/path/to/your-project"]
    }
  }
}
```

<details>
<summary>暴露的 MCP 工具（9 个）</summary>

- `keel_rules_list` · `keel_rules_match` · `keel_rules_check`
- `keel_task_list` · `keel_task_create` · `keel_task_show`
- `keel_verify`
- `keel_mem_search` · `keel_mem_sessions`

</details>

Windows 配置示例与双端对照测试见 [crates/keel-mcp/README.md](./crates/keel-mcp/README.md)。

## 仓库目录结构

```text
.keel/
  config.yaml        # 校验门禁与可选额外记忆目录
  rules/             # 带 YAML frontmatter 的按路径规则
  tasks/             # 可选任务目录（meta.json + spec.md）
AGENTS.md            # 托管块（<!-- KEEL:START --> ... <!-- KEEL:END -->）
.agents/skills/      # keel-plan, keel-verify, keel-distill, keel-mem
```

## 开源协议

[MIT](./LICENSE)
