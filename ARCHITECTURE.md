# Architecture

Keel is a single pure-Rust executable (`keel`) that serves both the CLI and the stdio MCP server from one codebase.

## Design choices

Keel assumes your coding tool (Claude Code, Codex, Cursor, OpenCode, Antigravity) already handles planning, code search, and sub-agent scheduling well. Keel stays strictly in the repository contract, local memory, and verification layer:

- No custom sub-agent definitions or dispatch rules.
- No vendor `PreToolUse` / `UserPromptSubmit` hooks.
- No Python or Node.js runtime required to run `keel` (the npm package `@pdxxxx/keel` is a zero-dependency launcher that downloads the SHA-256 verified Rust binary from GitHub Releases).
- No built-in LLM API calls.
- No task directory required for small edits.

## Code layout

- `Cargo.toml`, `src/` — single Rust crate producing `keel`:
  - `main.rs` / `cli.rs` — CLI parser (`-C/--root`, `--json`, clean single-line `stderr` diagnostics)
  - `mcp.rs` & `mcp_root.rs` — stdio JSON-RPC 2.0 MCP server (`2024-11-05`, 13 tools, per-call `root`, `initialize` `rootUri`/`workspaceFolders`, homeless-cwd write guard, `--lock-root`)
  - `init.rs` & `doctor.rs` — non-destructive `init`, template-only `update`, strict config & marker `doctor`
  - `rules.rs`, `tasks.rs`, `verify.rs`, `hotspots.rs`, `mem.rs`, `migrate.rs`, `ablate.rs`, `snapshot.rs`, `config.rs`, `git.rs`, `paths.rs`
- `tests/` — real child-process CLI (`tests/cli.rs`) and stdio MCP (`tests/stdio.rs`) integration suites.
- `npm/` — zero-dependency npm wrapper (`npm/install.js`, `npm/bin/keel.js`, `npm/test.mjs`).
- `docs/` — standalone VitePress documentation site deployed to GitHub Pages.

## On-disk formats

### Rules (`.keel/rules/**/*.md`)

```markdown
---
id: auth-token-rotation
summary: Rotate refresh tokens atomically
severity: invariant
paths:
  - src/auth/**
---

Explain why this invariant exists.
```

- `severity`: `invariant`, `guideline`, or `adr` (defaults to `guideline`).
- `paths`: POSIX globs (`*`, `**`, `**/`, `?`). Empty or omitted means global.
- Frontmatter starts with `---` on line 1 and closes only on a standalone `---` line (LF, CRLF, or EOF).
- `keel rules check --stale` warns on dead path globs, rules over 80 lines, and more than 5 global rules (`--strict` fails on warnings).

### Tasks (`.keel/tasks/<YYYY-MM-DD>-<slug>/`)

- `meta.json`: `id`, `title`, `status` (`open` | `done`), `createdAt`, `updatedAt`.
- `spec.md`: single living document for goals, non-goals, acceptance criteria, and decisions.
- `research/`: optional persisted notes.
- Archived tasks live under `.keel/tasks/archive/<id>/`. Task IDs with `..` or absolute paths are rejected.

### Verification gates (`.keel/config.yaml`)

```yaml
version: 1
verify:
  gates:
    - name: api-test
      command: pnpm --filter api test
      paths:
        - packages/api/**
      timeoutMs: 120000
      maxOutputChars: 16000
memory:
  sources: []
```

- `timeoutMs` defaults to `120000` (`0` disables timeout). Timed-out gates terminate their process tree and report `exitCode: 124` with `timedOut: true`.
- `maxOutputChars` defaults to `16000` (`0` disables truncation). Oversized logs keep the tail (preserving error summaries) with a truncation header.
- Invalid or unknown keys in `.keel/config.yaml` are caught by `keel doctor` and fail closed before gates run.

### Memory (`src/mem.rs`)

Streaming, read-only scan across seven sources:

- `keel`: `.keel/tasks/` and `.keel/rules/` Markdown files
- `claude`: `~/.claude/projects/**/*.jsonl`
- `codex`: `~/.codex/sessions/**/*.jsonl`
- `opencode`: `~/.local/share/opencode/**/*.jsonl`
- `antigravity`: `~/.gemini/antigravity/brain/*/.system_generated/logs/transcript.jsonl`
- `gemini`: `~/.gemini/tmp/*/chats/*.json`
- `custom`: directories in `.keel/config.yaml` (`memory.sources`) and `KEEL_MEM_DIRS`

Supports `--scope project | all`: `project` restricts results to sessions matching the active repository path; `all` searches everything while boosting current-project matches.

### External SHA-256 snapshots (`src/snapshot.rs`)

`keel ablate` (`~/.keel-ablate/<repo-hash>/`) and `keel migrate cleanup` (`~/.keel-migrate/<repo-hash>/`) share one transaction-safe snapshot engine:

1. Rejects symlinks anywhere along workspace or backup paths.
2. Writes and SHA-256 verifies every backup file before mutating the repository.
3. Strips managed blocks in `AGENTS.md` / `CLAUDE.md` and Trellis hooks in JSON settings without disturbing unrelated user content.
4. Verifies every path against post-operation hashes before restoring (refusing conflicts unless `--force` is passed).

### Multi-repo MCP root resolution (`src/mcp_root.rs`)

Per-call precedence (highest to lowest):

1. `arguments.root` (accepted on all 13 tools; resolves upward `.keel/` -> `.git/` -> given dir)
2. Startup `--root <path>`
3. `initialize` `params.rootUri` or `params.workspaceFolders[0].uri` (when `--root` was omitted)
4. Process `cwd` upward discovery

When no explicit root is provided and `cwd` has neither `.keel/` nor `.git/`, mutating and verification tools (`keel_task_create`, `keel_task_update`, `keel_rules_add`, `keel_verify`) refuse to run instead of writing into a homeless aggregator directory. Pass `--root <dir> --lock-root` to restrict all calls within one canonical directory tree.
