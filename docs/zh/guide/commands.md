# 命令参考

所有命令均支持 `--json` 输出。

## `keel init`

在当前目录初始化 Keel。

```sh
keel init [-f | --force] [--no-skills] [--no-detect] [--json]
```

- 自动识别 `package.json`、`Cargo.toml`、`go.mod`、`pyproject.toml` 中的常见校验脚本并写入 `.keel/config.yaml`。
- 幂等写入 `AGENTS.md` 中的 `<!-- KEEL:START --> ... <!-- KEEL:END -->` 托管块。
- 安装 5 个宿主无关标准 Skill：`keel-plan`、`keel-verify`、`keel-distill`、`keel-mem`、`keel-migrate`。

## `keel rules`

管理 `.keel/rules/` 下的按路径 Markdown 规则。

```sh
keel rules list [--json]
keel rules match [paths...] [--git] [--staged] [--base <ref>] [--no-global] [--json]
keel rules add <id> --summary <text> [--severity invariant|guideline|adr] [--paths <glob...>] [-f] [--json]
keel rules check [--stale] [--strict] [--json]
```

- `match` 传入 `--git`、`--staged` 或 `--base <ref>` 时会自动合并 Git 变更文件路径进行匹配。
- `check --stale` 会检查匹配不到任何仓库文件的死 glob、超过 80 行的过长规则以及超过 5 条的全局规则；`--strict` 会在出现告警时也返回非零退出码。

## `keel task`

管理 `.keel/tasks/` 下的可选单文档任务。

```sh
keel task create <title> [--slug <slug>] [--json]
keel task list [--all] [--status open|done] [--json]
keel task show <id> [--json]
keel task done <id> [--json]
keel task reopen <id> [--json]
keel task archive <id> [--json]
```

每个任务包含 `meta.json` 和单份活文档 `spec.md`（以及可选的 `research/` 调研目录）。归档任务移动到 `.keel/tasks/archive/<id>/`，可用 `keel task show archive/<id>` 读取。

## `keel verify`

按顺序运行 `.keel/config.yaml` 中配置的确定性检查门禁。

```sh
keel verify [-g | --gate <name...>] [--git] [--staged] [--base <ref>] [--fail-fast] [--json]
```

- 不带参数时运行全部门禁。
- 带 `--git`、`--staged` 或 `--base <ref>`（且未显式指定 `--gate`）时，仅运行全局门禁和 `paths` 命中变更文件的门禁。

## `keel hotspots`

按近期提交频率与 bugfix/revert 次数对仓库内现存文件排序。

```sh
keel hotspots [--commits <n>] [--limit <n>] [--json]
```

默认检查最近 100 次提交并返回 Top 10 现存文件（`score = fixTouches * 3 + touches`），自动跳过已删除文件。

## `keel mem`

只读检索 `.keel/` 文档与本地 AI 编程会话日志（Claude Code、Codex、OpenCode 及 `.keel/config.yaml` / `KEEL_MEM_DIRS` 指定的自定义目录）。

```sh
keel mem sources [--source <name>] [--json]
keel mem sessions [--source <name>] [-n | --limit <count>] [--json]
keel mem search <query> [--source <name>] [-n | --limit <count>] [--json]
keel mem show <sessionId> [-n | --limit <count>] [--json]
```

## `keel migrate`

盘点并安全清理冲突的旧 AI 框架配置（Trellis、`.cursorrules`、`CLAUDE.md`、Copilot、Windsurf、Spec-Kit、BMAD）。

```sh
keel migrate scan [--json]
keel migrate plan [--json]
keel migrate cleanup [--dry-run] [--state-dir <dir>] [--json]
keel migrate rollback [--dry-run] [--force] [--state-dir <dir>] [--json]
```

`cleanup` 修改仓库前会先做外部 SHA-256 备份，`rollback` 恢复前会做全路径冲突检测。

## `keel ablate` 与 `keel restore`

临时将所有 Keel 文件（`.keel/`、`.agents/skills/keel-*` 及 `AGENTS.md` 中的托管块）移入外部校验快照以做 A/B 对比，随后逐字节无损恢复。

```sh
keel ablate [--dry-run] [--state-dir <dir>] [--json]
keel ablate status [--state-dir <dir>] [--json]
keel restore [--dry-run] [--force] [--state-dir <dir>] [--json]
```

## `keel mcp`

启动内置 Node.js 版 stdio MCP 服务。

```sh
keel mcp
```
