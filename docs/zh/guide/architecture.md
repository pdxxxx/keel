# 架构与磁盘格式

## 设计边界

Keel 假设你正在使用的编码工具（Claude Code、Codex、Cursor、OpenCode、Antigravity）本身已经足够擅长规划、代码搜索和原生子 Agent 调度。因此 Keel 刻意不做以下事情：

- 不定义自定义子 Agent 角色或串行派发规则。
- 不拦截各平台私有 `PreToolUse` / `UserPromptSubmit` Hook。
- 不往用户仓库写入 Python 辅助脚本。
- 不在 CLI 里内置任何大模型 API 密钥依赖。
- 不强制为简单小改动创建任务目录。

## 代码结构

- `src/core/`：TypeScript 核心模块（`init`、`config`、`rules`、`glob`、`git`、`tasks`、`verify`、`hotspots`、`mem`、`migrate`、`ablate`、`snapshot`、`mcp`）。
- `src/cli.ts` & `bin/keel.js`：薄 CLI 入口封装。
- `crates/keel-mcp/`：独立 Rust stdio MCP 服务端，通过 `crates/keel-mcp/tests/parity.mjs` 与 TypeScript 版保持逐响应一致。

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
- `paths`：POSIX glob 数组（支持 `*`、`**`、`**/`、`?`）；为空或省略表示全局规则。
- Frontmatter 必须从第 1 行 `---` 开始，且只在独占一行的 `---` 处结束（支持 LF、CRLF、EOF）。

### 任务（`.keel/tasks/<YYYY-MM-DD>-<slug>/`）

- `meta.json`：`id`、`title`、`status`（`open` | `done`）、`createdAt`、`updatedAt`。
- `spec.md`：单份活文档（目标、Non-Goals、验收标准、关键决策）。
- `research/`：可选调研笔记目录。
- 归档任务位于 `.keel/tasks/archive/<id>/`。TS 与 Rust 两端均拒绝含 `..` 或绝对路径的非法 ID。

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
memory:
  sources: []
```

### 外部 SHA-256 快照（`ablate` 与 `migrate cleanup`）

`keel ablate`（`~/.keel-ablate/<repo-hash>/`）与 `keel migrate cleanup`（`~/.keel-migrate/<repo-hash>/`）共用同一套事务快照机制：

1. 在改动仓库任何文件前，先把原始字节备份到仓库外部并逐文件校验 SHA-256。
2. 在 `manifest.json` 记录操作后的目标哈希。
3. 剥离 `AGENTS.md` 中的托管块时完整保留用户的其他正文。
4. 执行 `restore` / `migrate rollback` 时先全量检测冲突，若期间有路径被修改则拒绝覆盖（除非显式传入 `--force`）。
