# MCP 服务

`keel mcp` 直接运行在单文件 Rust `keel` 二进制中，通过 stdio JSON-RPC 2.0（`protocolVersion: "2024-11-05"`）暴露 **13 个工具**，零外部运行时依赖。

## 多仓库与 MCP 聚合服务支持

单个常驻 `keel mcp` 进程即可安全并发服务多个仓库（完美适配 Magpie、MetaMCP、OneMCP 等 MCP 聚合器或全局 Codex 配置）。全部 13 个工具均接受可选参数 `root`。

单次请求的 `root` 解析优先级（从高到低）：

1. **`arguments.root`**（最高优先级——传入目标仓库根目录或仓库内任意子目录，内部向上解析 `.keel/` → `.git/` → 传入目录）。
2. **启动参数 `--root <path>`**
3. **MCP `initialize` 工作区声明**（未传 `--root` 时读取 `params.rootUri` 或 `params.workspaceFolders[0].uri`）。
4. **进程 `cwd`** 向上发现。

内置安全护栏：
- **无家目录防误写保护**：当未提供显式 root 且 `cwd` 向上既无 `.keel/` 也无 `.git/` 时，写操作与验证工具（`keel_task_create`、`keel_task_update`、`keel_rules_add`、`keel_verify`）会直接报错提示传入 `root`，绝不会把 `.keel/` 写进 `~` 或聚合器目录。
- **可选 `--lock-root` 沙箱锁**：启动时传入 `keel mcp --root /path/to/repo --lock-root`，会拒绝任何真实路径越出锁定目录树的 `arguments.root`。

## 暴露的 13 个工具

| 工具 | 参数 | 说明 |
| --- | --- | --- |
| `keel_rules_list` | `{ root? }` | 列出 `.keel/rules/` 下全部规则 |
| `keel_rules_match` | `{ paths?, git?, includeGlobal?, root? }` | 拉取命中指定路径或当前 git diff 的规则 |
| `keel_rules_add` | `{ id, summary, severity?, paths?, body?, force?, root? }` | 新增一条按路径作用域规则或 ADR |
| `keel_rules_check` | `{ stale?, root? }` | 校验 frontmatter/ID，可选检查死路径与超长规则 |
| `keel_task_list` | `{ status?, includeArchived?, root? }` | 列出活跃（及可选归档）任务 |
| `keel_task_create` | `{ title, slug?, root? }` | 创建任务目录、`meta.json` 与 `spec.md` |
| `keel_task_show` | `{ id, root? }` | 读取任务元数据与完整 `spec.md` |
| `keel_task_update` | `{ id, action: "done"\|"reopen"\|"archive", root? }` | 更新任务状态或归档任务 |
| `keel_verify` | `{ gates?, git?, failFast?, root? }` | 运行门禁（自带超时强杀与末尾日志截断） |
| `keel_hotspots` | `{ commits?, limit?, root? }` | 按近期 bugfix 频次对现存文件排序 |
| `keel_mem_search` | `{ query, source?, scope?, limit?, root? }` | 流式检索 7 类本地记忆源 |
| `keel_mem_sessions` | `{ source?, scope?, limit?, root? }` | 按修改时间倒序列出最近会话摘要 |
| `keel_mem_show` | `{ sessionId, limit?, root? }` | 查看某条会话的完整对话轮次 |

## 客户端接入配置

### Codex CLI 全局安装

```sh
codex mcp add keel -- keel mcp
```

### 通用 `mcpServers` JSON（Claude Code / Cursor / OpenCode / Antigravity）

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

如需强制锁死在单个仓库目录：

```json
{
  "mcpServers": {
    "keel": {
      "command": "keel",
      "args": ["mcp", "--root", "/absolute/path/to/your-project", "--lock-root"]
    }
  }
}
```
