# MCP 服务（Node 与 Rust）

Keel 提供两套协议完全一致的 stdio MCP 实现（`protocolVersion: "2024-11-05"`）：

- **Node.js 版（`keel mcp`）**：随 `@pdxxxx/keel` 自带，无需额外安装。
- **原生 Rust 版（`keel-mcp`）**：位于 `crates/keel-mcp/`，单文件约 2 MiB，不依赖 Node.js，启动更快、流式扫描大体积 JSONL 会话更省内存。

CI 在 Linux 与 Windows 上对两版运行 **61 项 stdio 响应对照测试**（`crates/keel-mcp/tests/parity.mjs`），保证行为一致。

## 性能实测（Rust vs Node）

本机 Windows 热文件缓存下每次启动新进程实测：

| 项目 | 原生 Rust（`keel-mcp`） | Node.js（`keel mcp`） |
| --- | --- | --- |
| stdio 握手中位耗时 | **3.2 ms** | — |
| 检索 98,304 条 JSONL 记录（前 15 条） | **43.2 ms** | 237.8 ms |
| 运行依赖与体积 | **~1.95 MiB 单文件** | 需 Node >= 20 |

## 暴露的 9 个工具

| 工具 | 参数 | 说明 |
| --- | --- | --- |
| `keel_rules_list` | `{}` | 列出 `.keel/rules/` 下全部规则 |
| `keel_rules_match` | `{ paths?, git?, includeGlobal? }` | 拉取命中指定路径或当前 git diff 的规则 |
| `keel_rules_check` | `{ stale? }` | 校验 frontmatter/ID，并可选检查死路径与超长规则 |
| `keel_task_list` | `{ status?, includeArchived? }` | 列出活跃（及可选归档）任务 |
| `keel_task_create` | `{ title, slug? }` | 创建任务目录、`meta.json` 与 `spec.md` |
| `keel_task_show` | `{ id }` | 读取任务元数据与完整 `spec.md`（支持 `archive/<id>`） |
| `keel_verify` | `{ gates?, git?, failFast? }` | 运行门禁（支持 `git: true` 按 `paths` 增量筛选） |
| `keel_mem_search` | `{ query, source?, limit? }` | 检索本地 AI 会话日志与 `.keel/` 文档 |
| `keel_mem_sessions` | `{ source?, limit? }` | 按修改时间倒序列出最近会话摘要 |

## 获取原生 Rust 二进制

使用 Rust 1.85+ 从源码全局安装：

```sh
cargo install --path crates/keel-mcp --locked
```

或从 [GitHub Releases](https://github.com/pdxxxx/keel/releases) 直接下载对应平台预编译文件（`keel-mcp-linux-x86_64`、`keel-mcp-macos-arm64`、`keel-mcp-windows-x86_64.exe`）。

## 客户端接入配置

### Codex CLI 全局安装

不写死 `--root`，`keel-mcp` 会随 Codex 打开的项目目录自动向上定位 `.keel/` 或 `.git/`：

```sh
codex mcp add keel -- /path/to/keel-mcp
```

### 通用 `mcpServers` JSON（Claude Code / Cursor / OpenCode / Antigravity）

使用原生 Rust 版：

```json
{
  "mcpServers": {
    "keel": {
      "command": "/absolute/path/to/keel-mcp",
      "args": ["--root", "/absolute/path/to/your-project"]
    }
  }
}
```

或直接使用 Node.js 版 CLI：

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
