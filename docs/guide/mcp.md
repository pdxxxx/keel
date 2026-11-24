# MCP Server

`keel mcp` runs directly inside the single Rust `keel` binary over stdio JSON-RPC 2.0 (`protocolVersion: "2024-11-05"`), exposing **13 tools** with zero external runtime dependencies.

## Multi-repo & MCP aggregator support

A single long-lived `keel mcp` process can safely serve multiple repositories concurrently (e.g. behind Magpie, MetaMCP, OneMCP, or a global Codex config). Every tool accepts an optional `root` parameter.

Root resolution precedence per request:

1. **`arguments.root`** (highest priority — pass the target repo root or any directory inside it; resolves upward `.keel/` → `.git/` → directory).
2. **Startup `--root <path>`**
3. **MCP `initialize` workspace** (`params.rootUri` or `params.workspaceFolders[0].uri`, when `--root` is omitted).
4. **Process `cwd`** upward discovery.

Safety guards:
- **Homeless directory protection**: when no explicit root is provided and `cwd` has neither `.keel/` nor `.git/`, mutating and verification tools (`keel_task_create`, `keel_task_update`, `keel_rules_add`, `keel_verify`) refuse to run instead of writing into `~` or an aggregator install folder.
- **Optional `--lock-root`**: launch `keel mcp --root /path/to/repo --lock-root` to reject any `arguments.root` whose canonical path escapes the locked repository.

## Exposed tools (13)

| Tool | Parameters | Purpose |
| --- | --- | --- |
| `keel_rules_list` | `{ root? }` | List all rules under `.keel/rules/` |
| `keel_rules_match` | `{ paths?, git?, includeGlobal?, root? }` | Pull rules matching paths or current git diff |
| `keel_rules_add` | `{ id, summary, severity?, paths?, body?, force?, root? }` | Add a scoped invariant, guideline, or ADR |
| `keel_rules_check` | `{ stale?, root? }` | Validate frontmatter/IDs and optionally flag stale/long rules |
| `keel_task_list` | `{ status?, includeArchived?, root? }` | List active (and optionally archived) tasks |
| `keel_task_create` | `{ title, slug?, root? }` | Create task directory with `meta.json` and `spec.md` |
| `keel_task_show` | `{ id, root? }` | Read task metadata and full `spec.md` |
| `keel_task_update` | `{ id, action: "done"\|"reopen"\|"archive", root? }` | Update task status or archive a task |
| `keel_verify` | `{ gates?, git?, failFast?, root? }` | Run configured gates with timeout and output-cap protection |
| `keel_hotspots` | `{ commits?, limit?, root? }` | Rank existing files by recent bugfix churn |
| `keel_mem_search` | `{ query, source?, scope?, limit?, root? }` | Streaming search across 7 local memory sources |
| `keel_mem_sessions` | `{ source?, scope?, limit?, root? }` | List recent local sessions sorted by mtime |
| `keel_mem_show` | `{ sessionId, limit?, root? }` | Read turns from a specific memory session |

## Client configuration

### Codex CLI (global)

```sh
codex mcp add keel -- keel mcp
```

### Standard `mcpServers` JSON (Claude Code, Cursor, OpenCode, Antigravity)

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

To lock an instance to a single repository:

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
