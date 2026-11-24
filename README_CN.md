<div align="center">

<img src="./assets/keel.png" width="84" height="84" alt="keel" />

# keel

**面向 AI 编程的仓库规范、任务记录、会话检索与验证门禁工具。**

<p>
  <a href="https://www.npmjs.com/package/@pdxxxx/keel"><img src="https://img.shields.io/npm/v/%40pdxxxx%2Fkeel?style=flat-square&color=cb3837" alt="npm version" /></a>
  <a href="https://github.com/pdxxxx/keel/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/pdxxxx/keel/ci.yml?style=flat-square&label=CI" alt="CI" /></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-2563eb?style=flat-square" alt="MIT License" /></a>
  <img src="https://img.shields.io/badge/rust-1.85%2B-dea584?style=flat-square&logo=rust&logoColor=black" alt="Rust 1.85+" />
</p>

<p>
  <a href="https://pdxxxx.github.io/keel/zh/">官网文档</a> ·
  <a href="./README.md">English</a> ·
  <a href="./ARCHITECTURE.md">Architecture</a> ·
  <a href="./CHANGELOG.md">Changelog</a>
</p>

</div>

---

Keel 是一个单文件纯 Rust 可执行程序（约 3.6 MiB），把项目规则、可选任务文档和校验命令以纯文本存在 `.keel/` 下，跨 7 类本地数据源检索历史会话，并通过 stdio MCP 暴露 13 个工具。

它不接管子 Agent 调度，不往编辑器里装私有 Hook，也不绑定特定 AI 工具。

## 安装

```sh
# 推荐：自动下载对应平台预编译二进制并校验 SHA-256
npm install -g @pdxxxx/keel@latest
```

或使用 Rust 1.85+ 从源码直接安装：

```sh
cargo install --git https://github.com/pdxxxx/keel --locked
```

Linux（`x86_64`、`aarch64`）、macOS（`arm64`、`x86_64`）与 Windows（`x86_64`）预编译单文件也可从 [GitHub Releases](https://github.com/pdxxxx/keel/releases) 直接下载。

## 快速开始

```sh
# 1. 初始化 .keel/、AGENTS.md 托管块与标准 .agents/skills/
keel init

# 2. 添加一条按路径生效的规则
keel rules add auth-rotation \
  --summary "Rotate refresh tokens atomically" \
  --paths "src/auth/**"

# 3. 查找命中当前 git diff 的规则
keel rules match --git

# 4. 检索当前项目相关的历史会话和 .keel/ 文档
keel mem search "refresh token" --scope project

# 5. 只运行命中当前变更的检查门禁（去掉 --git 跑全量）
keel verify --git
```

## 老仓库接入、升级与迁移

```sh
keel hotspots                    # 找出高频修复文件，优先给暗坑补规则
keel doctor                      # 检查 config.yaml 字段错误、AGENTS.md 标记、git 与失效规则
keel update                      # 安全刷新 AGENTS.md 块与内置 skills，不动你的 config.yaml
keel migrate scan                # 盘点 Trellis、.cursorrules、CLAUDE.md 等旧配置
keel migrate cleanup --dry-run   # 预览安全清理（自动备份，可 `keel migrate rollback` 回滚）
```

## 命令一览

全局选项：`-C, --root <PATH>` 与 `--json`。

| 命令 | 说明 |
| --- | --- |
| `keel init` | 创建 `.keel/`、自动探测校验脚本、更新 `AGENTS.md` 并安装 5 个标准 skills（除非显式 `--reset-config`，绝不覆盖已有 `config.yaml`）。 |
| `keel update` | 安全刷新托管模板（`AGENTS.md` 块、`rules/index.md`、`keel-*` skills），不碰用户配置、规则和任务。 |
| `keel doctor` | 严格校验 `.keel/config.yaml` 字段与类型、检查 `AGENTS.md` 标记、`git` 及过期规则。 |
| `keel rules` | `list`、`match [paths...] [--git \| --staged \| --base <ref>]`、`add <id>`、`check [--stale \| --strict]`。 |
| `keel task` | `create`、`list`、`show`、`done`、`reopen`、`archive`，按需用单个 `spec.md` 记录复杂任务。 |
| `keel verify` | 运行 `.keel/config.yaml` 中的门禁（支持 `--gate`、`--git`、`--staged`、`--base <ref>`、`--fail-fast`），自带超时强杀与输出截断。 |
| `keel hotspots` | 按近期改动与 bugfix 频率列出高风险现存文件（`--commits`、`--limit`）。 |
| `keel mem` | `sources`、`sessions`、`search`、`show`（支持 `--scope project \| all`），跨 `.keel/`、Claude Code、Codex、OpenCode、Antigravity、Gemini 与自定义目录流式检索。 |
| `keel migrate` | `scan`、`plan`、`cleanup [--dry-run]`、`rollback`，由外部 SHA-256 快照保护。 |
| `keel ablate` / `restore` | 临时移出 Keel 文件做 A/B 对比，随后逐字节无损恢复。 |
| `keel mcp` | 启动 stdio MCP 服务（13 个工具，支持按次 `root` 切换多仓库与 `--lock-root`）。 |

## MCP 接入

单个常驻 `keel mcp` 进程即可安全服务多个仓库（完美适配 MCP 聚合服务）。全部 13 个工具均支持最高优先级可选参数 `root`。

**Codex CLI 全局安装**

```sh
codex mcp add keel -- keel mcp
```

**通用 `mcpServers` JSON**（Claude Code / Cursor / OpenCode / Antigravity）

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

如需强制锁死在单个仓库目录，传入 `--root <path> --lock-root`。

<details>
<summary>暴露的 MCP 工具（13 个）</summary>

- `keel_rules_list` · `keel_rules_match` · `keel_rules_add` · `keel_rules_check`
- `keel_task_list` · `keel_task_create` · `keel_task_show` · `keel_task_update`
- `keel_verify` · `keel_hotspots`
- `keel_mem_search` · `keel_mem_sessions` · `keel_mem_show`

</details>

## 仓库目录结构

```text
.keel/
  config.yaml        # 校验门禁（支持 paths、timeoutMs、maxOutputChars）与额外记忆目录
  rules/             # 带 YAML frontmatter 的按路径规则
  tasks/             # 可选任务目录（meta.json + spec.md）
AGENTS.md            # 托管块（<!-- KEEL:START --> ... <!-- KEEL:END -->）
.agents/skills/      # keel-plan, keel-verify, keel-distill, keel-mem, keel-migrate
```

## 开源协议

[MIT](./LICENSE)
