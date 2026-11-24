# 命令参考

全局选项（适用于所有命令）：

- `-C, --root <PATH>`：指定目标仓库或其内部任意子目录
- `--json`：输出机器可读 JSON

## `keel init`

```sh
keel init [-f | --force] [--reset-config] [--no-skills] [--no-detect]
```

创建 `.keel/`、自动探测常见校验脚本（`package.json`、`Cargo.toml`、`go.mod`、`pyproject.toml`）、严格成对校验并更新 `AGENTS.md` 托管块，以及安装 5 个标准 skills（`keel-plan`、`keel-verify`、`keel-distill`、`keel-mem`、`keel-migrate`）。即使传 `--force`，除非显式 `--reset-config`，不会覆盖已有 `.keel/config.yaml`。

## `keel update`

```sh
keel update [--dry-run] [--no-skills]
```

仅刷新 Keel 托管模板（`AGENTS.md` 托管块、`.keel/rules/index.md`、`.agents/skills/keel-*/SKILL.md`），绝不触碰 `.keel/config.yaml`、用户规则或任务。

## `keel doctor`

```sh
keel doctor
```

严格校验 `.keel/` 结构与 `.keel/config.yaml`（捕获拼写错误字段、非法类型、空命令与重名门禁），检查 `AGENTS.md` 标记成对完整性、检查 `git` 可用性，并运行 `rules check --stale`。有错误时返回非零退出码。

## `keel rules`

```sh
keel rules list
keel rules match [paths...] [--git] [--staged] [--base <ref>] [--no-global]
keel rules add <id> --summary <text> [--severity invariant|guideline|adr] [--paths <glob...>] [-f]
keel rules check [--stale] [--strict]
```

`check --stale` 告警死路径 glob、超过 80 行的过长规则与超过 5 条的全局规则（`--strict` 遇告警即返回非零退出码）。

## `keel task`

```sh
keel task create <title> [--slug <slug>]
keel task list [--all] [--status open|done]
keel task show <id>
keel task done <id>
keel task reopen <id>
keel task archive <id>
```

归档任务位于 `.keel/tasks/archive/<id>/`，可用 `keel task show archive/<id>` 读取。

## `keel verify`

```sh
keel verify [-g | --gate <name...>] [--git] [--staged] [--base <ref>] [--fail-fast]
```

按配置顺序运行门禁。支持 `paths` 增量筛选（`--git` / `--staged` / `--base`）、单门禁 `timeoutMs`（默认 120s，超时终止进程树并返回 `exitCode: 124`）及 `maxOutputChars`（默认 16,000，保留末尾日志）。

## `keel hotspots`

```sh
keel hotspots [--commits <n>] [--limit <n>]
```

按近期提交频率与 bugfix/revert 次数对现存文件排名（`score = fixTouches * 3 + touches`），嵌套 monorepo 下同样精准对齐路径。

## `keel mem`

```sh
keel mem sources [--source <name>]
keel mem sessions [--source <name>] [--scope project|all] [-n | --limit <count>]
keel mem search <query> [--source <name>] [--scope project|all] [-n | --limit <count>]
keel mem show <sessionId> [-n | --limit <count>]
```

原地流式只读检索 `keel`、`claude`、`codex`、`opencode`、`antigravity`、`gemini` 与 `custom` 共 7 类本地数据源。

## `keel migrate`

```sh
keel migrate scan
keel migrate plan
keel migrate cleanup [--dry-run] [--state-dir <dir>]
keel migrate rollback [--dry-run] [--force] [--state-dir <dir>]
```

盘点并安全清理冲突旧框架配置（完整覆盖 Trellis 的 `.trellis/`、`AGENTS.md`、`CLAUDE.md`、`.agents/skills/trellis-*`、`.claude/`、`.codex/`、`.cursor/`、`.gemini/`，以及 `.cursorrules`、Windsurf、Spec-Kit、BMAD），外部 SHA-256 备份保护、支持无损回滚。

## `keel ablate` 与 `keel restore`

```sh
keel ablate [--dry-run] [--state-dir <dir>]
keel ablate status [--state-dir <dir>]
keel restore [--dry-run] [--force] [--state-dir <dir>]
```

将 Keel 相关文件临时移入外部 SHA-256 校验快照以做 A/B 对比，随后逐字节无损恢复。

## `keel mcp`

```sh
keel mcp [--root <path>] [--lock-root]
```

启动内置 stdio JSON-RPC 2.0 MCP 服务，暴露全部 13 个工具。
