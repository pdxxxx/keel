# Architecture & Formats

## Design boundaries

Keel assumes your coding tool (Claude Code, Codex, Cursor, OpenCode, Antigravity) already handles planning, code search, and sub-agent execution well. Consequently, Keel deliberately avoids:

- Custom sub-agent definitions or dispatch rules.
- Vendor-specific `PreToolUse` / `UserPromptSubmit` hooks.
- Python helper scripts inside user repositories.
- Built-in LLM API calls inside the CLI.
- Requiring a task directory for trivial changes.

## Repository layout

- `src/core/` — TypeScript domain modules (`init`, `config`, `rules`, `glob`, `git`, `tasks`, `verify`, `hotspots`, `mem`, `migrate`, `ablate`, `snapshot`, `mcp`).
- `src/cli.ts` & `bin/keel.js` — thin CLI entrypoint.
- `crates/keel-mcp/` — standalone Rust stdio MCP server verified against `src/core/mcp.ts` via `crates/keel-mcp/tests/parity.mjs`.

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

### Tasks (`.keel/tasks/<YYYY-MM-DD>-<slug>/`)

- `meta.json`: `id`, `title`, `status` (`open` | `done`), `createdAt`, `updatedAt`.
- `spec.md`: single living document (Goal, Non-Goals, Acceptance Criteria, Decisions).
- `research/`: optional notes directory.
- Archived tasks live at `.keel/tasks/archive/<id>/`. Both TS and Rust reject task IDs with `..` or absolute paths.

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
memory:
  sources: []
```

### External SHA-256 snapshots (`ablate` & `migrate cleanup`)

Both `keel ablate` (`~/.keel-ablate/<repo-hash>/`) and `keel migrate cleanup` (`~/.keel-migrate/<repo-hash>/`) share the same snapshot engine:

1. Write and SHA-256 verify every backup byte outside the repository before mutating any file.
2. Record post-operation hashes in `manifest.json`.
3. Strip managed marker blocks in `AGENTS.md` without altering surrounding user text.
4. On `restore` / `migrate rollback`, verify every managed path against post-operation hashes before writing a single byte (refusing on conflict unless `--force` is passed).
