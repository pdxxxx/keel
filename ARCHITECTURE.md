# Architecture

This document explains how Keel is laid out, what file formats it owns, and why certain features were left out.

## Design choices

Keel assumes the coding tool you already use (Claude Code, Codex, Cursor, OpenCode, etc.) handles planning, file search, and sub-agent execution on its own. Trying to override that with vendor-specific hooks or custom sub-agent topologies creates maintenance churn every time a host changes its API and often fights native parallelism and prompt caching.

Because of that, Keel intentionally avoids:

- Custom sub-agent definitions or dispatch rules.
- Vendor-specific `PreToolUse` / `UserPromptSubmit` hooks.
- Python helper scripts inside user repositories (the CLI is Node.js >= 20; the optional native MCP server is Rust).
- Built-in LLM API dependencies inside the CLI.
- Requiring a task folder for small edits.

Everything Keel does lives in plain files under `.keel/`, a short managed block in `AGENTS.md`, five skill files in `.agents/skills/`, and stateless read/write commands.

## Code layout

- `src/core/`: core logic (`init`, `config`, `rules`, `glob`, `git`, `tasks`, `verify`, `hotspots`, `mem`, `migrate`, `ablate`, `snapshot`, `mcp`).
- `src/cli.ts` & `bin/keel.js`: CLI wrapper over `src/core/`.
- `src/templates/assets.ts`: templates written by `keel init`.
- `crates/keel-mcp/`: standalone Rust implementation of the stdio MCP server. Tested against the Node implementation via `crates/keel-mcp/tests/parity.mjs`.

## On-disk formats

### Rules (`.keel/rules/**/*.md`)

Rule files use YAML frontmatter followed by Markdown:

```markdown
---
id: auth-token-rotation
summary: Rotate refresh tokens atomically
severity: invariant
paths:
  - src/auth/**
---

Concurrent tab refreshes previously caused false-positive token revocation...
```

- `severity` is one of `invariant`, `guideline`, or `adr` (defaults to `guideline`).
- `paths` is a list of POSIX globs (`*`, `**`, `**/`, `?`). If omitted or empty, the rule is treated as global.
- Frontmatter must start with `---` on line 1 and close with a standalone `---` line.
- `.keel/rules/index.md` is ignored by `list` and `check` unless it declares an explicit `id` in frontmatter.

`keel rules match` returns only the metadata index (`id`, `file`, `summary`, `severity`, `paths`, `global`) so an agent can open only the files that actually apply. `keel rules check --stale` additionally flags path globs matching zero repo files, rules longer than 80 lines, and repositories with more than 5 global rules (`--strict` fails on warnings).

### Tasks (`.keel/tasks/<YYYY-MM-DD>-<slug>/`)

Each task folder holds:

- `meta.json`: `id`, `title`, `status` (`open` or `done`), `createdAt`, `updatedAt`.
- `spec.md`: a single Markdown file for goals, non-goals, acceptance criteria, and decisions.
- `research/`: optional directory for notes that should survive chat compaction.

`keel task archive <id>` sets status to `done` and moves the folder into `.keel/tasks/archive/<id>/`. Both TS and Rust implementations reject task IDs containing `..` or absolute paths.

### Verification gates (`.keel/config.yaml`)

```yaml
version: 1
verify:
  gates:
    - name: api-test
      command: pnpm --filter api test
      paths:
        - packages/api/**
    - name: typecheck
      command: pnpm typecheck
memory:
  sources: []
```

`keel verify` runs gates sequentially in the configured order, captures stdout/stderr, exit code, and elapsed milliseconds, and exits non-zero if any gate fails. When `--git`, `--staged`, or `--base <ref>` is passed (or `git: true` over MCP) without explicit `--gate` names, only global gates (no `paths`) and gates whose `paths` match changed files are executed; skipped gate names are reported in `skipped`.

### Hotspots (`src/core/hotspots.ts`)

`keel hotspots` reads recent commits from `git log`, skips deleted files, and ranks existing paths by `fixTouches * 3 + touches` so brownfield repos can prioritize which areas need rules or tests first.

### Memory (`src/core/mem.ts`, `crates/keel-mcp/src/mem.rs`)

Read-only scan across:

- `keel`: `.keel/tasks/` and `.keel/rules/` Markdown files.
- `claude`: `~/.claude/projects/**/*.jsonl`
- `codex`: `~/.codex/sessions/**/*.jsonl`
- `opencode`: `~/.local/share/opencode/**/*.jsonl`
- `custom`: directories listed in `.keel/config.yaml` (`memory.sources`) and the `KEEL_MEM_DIRS` environment variable.

Malformed JSONL lines and unreadable directories are skipped silently. Search splits the query on whitespace, scores each turn by term coverage plus capped occurrence count, and returns top hits with short snippets.

### Snapshots, ablation, and migration (`src/core/snapshot.ts`, `ablate.ts`, `migrate.ts`)

`keel ablate` and `keel migrate cleanup` share the same external SHA-256 snapshot engine (`~/.keel-ablate/<repo-hash>/` and `~/.keel-migrate/<repo-hash>/`). Every target file is backed up and hash-verified before mutating the working tree, and `restore` / `migrate rollback` verify all paths for post-operation conflicts before writing anything back (unless `--force` is passed). Managed blocks in `AGENTS.md` are stripped without disturbing surrounding user text.

### MCP server (`src/core/mcp.ts`, `crates/keel-mcp/`)

Newline-delimited JSON-RPC 2.0 over stdio (`protocolVersion: "2024-11-05"`). Supports `initialize`, `notifications/*` (no response), `ping`, `tools/list`, and `tools/call`. All logs and diagnostics go to `stderr`; `stdout` carries protocol frames only.
