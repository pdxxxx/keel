# Architecture & Formats

Keel is a single pure-Rust executable (`keel`) serving both the CLI and the 13-tool stdio MCP server from one codebase.

## Design boundaries

Keel assumes your coding tool (Claude Code, Codex, Cursor, OpenCode, Antigravity) already handles planning, code search, and sub-agent execution well. Keel deliberately avoids:

- Custom sub-agent definitions or dispatch rules.
- Vendor-specific `PreToolUse` / `UserPromptSubmit` hooks.
- Python or Node.js runtimes for core execution (`@pdxxxx/keel` on npm is a zero-dependency launcher that downloads and SHA-256 verifies the prebuilt Rust binary).
- Built-in LLM API calls.
- Requiring a task directory for small changes.

## Repository layout

- `Cargo.toml`, `src/` — pure-Rust CLI, MCP server (`src/mcp.rs`, `src/mcp_root.rs`), and shared domain modules.
- `tests/` — real child-process integration suites (`tests/cli.rs`, `tests/stdio.rs`).
- `npm/` — zero-dependency npm installer (`npm/install.js`), launcher (`npm/bin/keel.js`), and `node:test` suite (`npm/test.mjs`).

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
- Frontmatter starts with `---` on line 1 and closes only on a standalone `---` line.

### Tasks (`.keel/tasks/<YYYY-MM-DD>-<slug>/`)

- `meta.json`: `id`, `title`, `status` (`open` | `done`), `createdAt`, `updatedAt`.
- `spec.md`: single living document (Goal, Non-Goals, Acceptance Criteria, Decisions).
- `research/`: optional notes directory.
- Archived tasks live at `.keel/tasks/archive/<id>/`. Task IDs with `..` or absolute paths are rejected.

### Verification gates (`.keel/config.yaml`)

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

Invalid or unknown config keys are surfaced by `keel doctor` and fail closed before running gates.

### External SHA-256 snapshots (`ablate` & `migrate cleanup`)

Both `keel ablate` (`~/.keel-ablate/<repo-hash>/`) and `keel migrate cleanup` (`~/.keel-migrate/<repo-hash>/`) share the same snapshot engine:

1. Rejects symlinks anywhere along workspace or backup paths.
2. Writes and SHA-256 verifies every backup byte outside the repository before mutating any file.
3. Strips managed marker blocks in `AGENTS.md` / `CLAUDE.md` and Trellis entries in JSON hook settings without touching unrelated user content.
4. On `restore` / `migrate rollback`, verifies every managed path against post-operation hashes before writing anything back (refusing conflicts unless `--force` is passed).
