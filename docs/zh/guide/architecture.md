# 架构与磁盘格式

Keel 是一个单文件纯 Rust 可执行程序（`keel`），CLI 与 13 工具 stdio MCP 服务共用同一套源码。

## 设计边界

Keel 假设你正在使用的编码工具（Claude Code、Codex、Cursor、OpenCode、Antigravity）本身擅长规划、代码搜索和原生子 Agent 调度，因此刻意不做以下事情：

- 不定义自定义子 Agent 角色或派发规则。
- 不拦截各平台私有 `PreToolUse` / `UserPromptSubmit` Hook。
- 核心运行不依赖 Python 或 Node.js（npm 上的 `@pdxxxx/keel` 是零依赖极薄启动器，安装时从 GitHub Releases 下载并校验 SHA-256）。
- 不内置任何大模型 API 密钥依赖。
- 不强制为小改动创建任务目录。

## 代码结构

- `Cargo.toml`、`src/`：纯 Rust CLI、MCP 服务（`src/mcp.rs`、`src/mcp_root.rs`）与共享领域模块。
- `tests/`：真实子进程 CLI（`tests/cli.rs`）与 stdio MCP（`tests/stdio.rs`）集成测试。
- `npm/`：零依赖 npm 下载器（`npm/install.js`）、启动器（`npm/bin/keel.js`）与 `node:test` 测试（`npm/test.mjs`）。

## 磁盘数据格式

### 规则（`.keel/rules/**/*.md`）

```markdown
---
id: auth-token-rotation
summary: Rotate refresh tokens atomically
severity: invariant
paths:
  - src/auth/**
---

说明为什么存在这条不变量。
```

- `severity`：`invariant`、`guideline` 或 `adr`（默认 `guideline`）。
- `paths`：POSIX glob 数组（`*`、`**`、`**/`、`?`）；为空或省略表示全局规则。
- Frontmatter 必须从第 1 行 `---` 开始，只在独占一行的 `---` 处结束。

### 任务（`.keel/tasks/<YYYY-MM-DD>-<slug>/`）

- `meta.json`：`id`、`title`、`status`（`open` | `done`）、`createdAt`、`updatedAt`。
- `spec.md`：单份活文档（目标、Non-Goals、验收标准、关键决策）。
- `research/`：可选调研笔记目录。
- 归档任务位于 `.keel/tasks/archive/<id>/`，拒绝含 `..` 或绝对路径的非法 ID。

### 校验门禁（`.keel/config.yaml`）

```yaml
version: 1
verify:
  gates:
    - name: typecheck
      command: pnpm typecheck
    - name: api-test
      command: pnpm --filter api test
      paths:
        - packages/api/**
      timeoutMs: 120000
      maxOutputChars: 16000
memory:
  sources: []
```

非法字段或类型错误会由 `keel doctor` 报出，并在执行门禁前 fail-closed 拦截。

### 外部 SHA-256 快照（`ablate` 与 `migrate cleanup`）

`keel ablate`（`~/.keel-ablate/<repo-hash>/`）与 `keel migrate cleanup`（`~/.keel-migrate/<repo-hash>/`）共用同一套快照引擎：

1. 拒绝工作区或备份路径中的任何符号链接。
2. 改动仓库前先将全部目标文件写入外部备份并校验 SHA-256。
3. 剥离 `AGENTS.md` / `CLAUDE.md` 托管块与 JSON Hook 配置中的 Trellis 项，完整保留用户的其他内容。
4. 恢复前全量检测冲突，若期间路径被改动则拒绝覆盖（除非显式传入 `--force`）。
