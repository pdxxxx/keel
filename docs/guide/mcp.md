# MCP Server (Node & Rust)

Keel includes two protocol-identical stdio MCP servers (`protocolVersion: "2024-11-05"`):

- **Node.js (`keel mcp`)** — bundled with `@pdxxxx/keel`, zero extra installation.
- **Native Rust (`keel-mcp`)** — single ~2 MiB executable in `crates/keel-mcp/`, no Node.js runtime needed, faster cold start and streaming JSONL memory search.

Both implementations are verified against each other in CI with a **61-case stdio parity suite** (`crates/keel-mcp/tests/parity.mjs`).

## Performance (Rust vs Node)

Measured on Windows with warm file cache (fresh process per run):

| Workload | Native Rust (`keel-mcp`) | Node.js (`keel mcp`) |
| --- | --- | --- |
| Stdio handshake median | **3.2 ms** | — |
| Search 98,304 JSONL turns (top 15) | **43.2 ms** | 237.8 ms |
| Binary size | **~1.95 MiB** | Requires Node >= 20 |

## Exposed tools (9)

| Tool | Parameters | Purpose |
| --- | --- | --- |
| `keel_rules_list` | `{}` | List all rules under `.keel/rules/` |
| `keel_rules_match` | `{ paths?, git?, includeGlobal? }` | Pull rules matching paths or current git diff |
| `keel_rules_check` | `{ stale? }` | Validate frontmatter/IDs and optionally warn on dead globs or long rules |
| `keel_task_list` | `{ status?, includeArchived? }` | List active (and optionally archived) tasks |
| `keel_task_create` | `{ title, slug? }` | Create task directory with `meta.json` and `spec.md` |
| `keel_task_show` | `{ id }` | Read task metadata and full `spec.md` (supports `archive/<id>`) |
| `keel_verify` | `{ gates?, git?, failFast? }` | Run configured gates (supports `git: true` incremental path filtering) |
| `keel_mem_search` | `{ query, source?, limit? }` | Search local AI transcripts and `.keel/` artifacts |
| `keel_mem_sessions` | `{ source?, limit? }` | List recent local sessions sorted by mtime |

## Installing the Rust binary

Install from source with Rust 1.85+:

```sh
cargo install --path crates/keel-mcp --locked
```

Or download a prebuilt binary (`keel-mcp-linux-x86_64`, `keel-mcp-macos-arm64`, `keel-mcp-windows-x86_64.exe`) from [GitHub Releases](https://github.com/pdxxxx/keel/releases).

## Client configuration

### Codex CLI (global)

Omit `--root` so `keel-mcp` automatically discovers `.keel/` or `.git/` from whichever workspace Codex opens:

```sh
codex mcp add keel -- /path/to/keel-mcp
```

### Standard `mcpServers` JSON (Claude Code, Cursor, OpenCode, Antigravity)

Using the native Rust binary:

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

Or using the Node.js CLI directly:

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
