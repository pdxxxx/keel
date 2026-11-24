# keel-mcp

Keel CLI 1.0.0 的原生 Rust MCP Server。直接读写仓库已有的 `.keel/` 文件；发布后只需一个可执行文件，不需要 Node、Python、数据库或后台服务。它只提供下面九个工具，不接管宿主的调度或 Hook。

## 构建

在本目录使用 Rust 1.85 或更高版本：

```sh
cargo build --release --locked
```

Windows 产物是 `target/release/keel-mcp.exe`，macOS 和 Linux 是 `target/release/keel-mcp`。可以单独复制到同平台的其他目录。需要 Git 变更匹配时，确保 `git` 在 `PATH` 上；验证门禁依赖的程序也必须在本机可用。

## 接入 MCP 客户端

以下是标准 `mcpServers` 配置。`command` 指向二进制，`--root` 指向**要操作的仓库**，两者可以在不同目录。Windows 示例：

```json
{
  "mcpServers": {
    "keel": {
      "command": "D:/Workspace/keel/crates/keel-mcp/target/release/keel-mcp.exe",
      "args": ["--root", "D:/Workspace/keel"]
    }
  }
}
```

macOS / Linux 示例（替换成实际绝对路径）：

```json
{
  "mcpServers": {
    "keel": {
      "command": "/usr/local/bin/keel-mcp",
      "args": ["--root", "/home/dev/my-project"]
    }
  }
}
```

若省略 `--root`，从进程工作目录向上找最近的 `.keel`；找不到才向上找 `.git`（包括 worktree 标记文件），最后使用工作目录。客户端工作目录不一定是编辑器中打开的项目，建议显式配置。

## 工具与数据

| 工具 | 行为 |
| --- | --- |
| `keel_rules_list` | 枚举 `.keel/rules/**/*.md`，返回 `id`、`file`、`summary`、`severity`、`paths`、`global`。 |
| `keel_rules_match` | 合并 `paths` 与可选 `git: true` 的未暂存、已暂存和未跟踪文件；`includeGlobal` 默认 `true`。 |
| `keel_rules_check` | 报告 frontmatter、必填字段、重复 ID、severity、paths 数组类型问题；无效报告带 `isError: true`。 |
| `keel_task_list` | 默认只列活动目录，可按 `status: "open" / "done"` 过滤，或用 `includeArchived: true` 包含归档。 |
| `keel_task_create` | 接受 `title` 与可选 `slug`。使用本地日期生成 ID，重名依次加 `-2`、`-3`；写入 `meta.json` 与 `spec.md`。 |
| `keel_task_show` | 读取 `id` 对应的元数据与完整 `spec`。归档任务可用 `archive/<id>` 读取。 |
| `keel_verify` | 执行 `.keel/config.yaml` 中的命令；可选 `gates`、`failFast`。按配置顺序运行，返回退出码、耗时和输出，失败设置 `isError: true`。 |
| `keel_mem_search` | 按 `query` 检索，支持 `source` 和 `limit`（默认 15），每条返回 `sessionId`、角色、分数、片段及可选时间戳。 |
| `keel_mem_sessions` | 支持 `source` 和 `limit`（默认 20），按文件最后修改时间从新到旧返回摘要。 |

协议为 `2024-11-05`，传输为 UTF-8 换行分隔的 JSON-RPC 2.0。握手名称为 `keel-mcp`，版本为 `1.0.0`。`stdout` 只写协议消息，通知不回复。工具结果位于 `result.content[0].text`；结构化结果是这个字符串中的 JSON，不另设数据字段。门禁的 stdout/stderr 只作为 `output` 数据返回。Windows 使用 `cmd.exe /D /S /C`，Unix 使用 `sh -c`。**仅对可信仓库启用验证**：命令以当前用户的权限运行。

规则使用 Keel 的 `*`、`**`、`**/` 和 `?`，不是 shell 的括号或花括号展开。对外展示的路径使用 `/`。任务时间戳是 UTC、精确到毫秒；日期前缀取本机本地日期。

### 记忆来源与排序

| `source` | 扫描位置 |
| --- | --- |
| `keel` | 仓库 `.keel/tasks/` 和 `.keel/rules/` 下的 Markdown（含归档与研究笔记） |
| `claude` | `~/.claude/projects/` 下的 JSONL |
| `codex` | `~/.codex/sessions/` 下的 JSONL |
| `opencode` | `~/.local/share/opencode/` 下的 JSONL |
| `custom` | `.keel/config.yaml` 的 `memory.sources` 和环境变量 `KEEL_MEM_DIRS` 下的 JSONL / Markdown |

配置中的相对目录以仓库为基准；环境变量中的相对目录以进程工作目录为基准，Windows 用 `;` 分隔，Unix 用 `:`。遵循 CLI 1.0.0 的深度约定：每个来源根目录下最多进入五层子目录。仓库文件显示相对路径，外部文件显示绝对路径。会话 ID 与同一台机器上的 CLI 一致，哈希输入是**本机格式**的绝对文件路径。

JSONL 使用缓冲逐行读取，损坏或非 UTF-8 的行被跳过，后面的有效行仍参与检索。不可读目录或文件不会中断其他来源。一次检索不保存全部历史内容，只保留文件清单、当前记录及前 `limit` 个结果。没有持久索引；文件变动后，下一次调用即可看到。评分为 `10 × 命中词数 / 查询词数 + 0.5 × min(出现次数, 10)`，保留两位小数；同分优先较新的会话，同一会话保持原始行顺序。搜索结果是本地文件内容，可能包含敏感信息；只连接可信客户端，或用 `source: "keel"` 将单次查询限制在仓库。

## 验证

在本目录运行（stdio 集成测试会使用本地 Git）：

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

测试会启动真实子进程，在临时仓库和假 HOME 中覆盖九个工具、错误响应、磁盘互读、输出隔离与损坏的会话。可额外与当前仓库的 Node 参考版逐请求对照；先在仓库根目录构建 CLI（`pnpm build`），再在本目录运行：

```sh
node tests/parity.mjs
```

对照脚本不使用真实聊天记录，不是运行二进制的依赖。它比较完整工具清单、结构化工具文本和错误，并让两个实现互读任务文件。比较时只忽略握手名称和非确定性的计时值。YAML 语法错误的具体解析器文案可能不同，两者都通过相同的工具错误或规则校验报告返回。
