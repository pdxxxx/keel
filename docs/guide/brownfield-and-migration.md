# Existing Repos & Migration

## Adopting Keel on a mature (1→N) codebase

Avoid generating broad style guides on day one. Roll Keel out in three steps:

### 1. Scoped verification gates with safety limits

Run `keel init` and configure module gates in `.keel/config.yaml`:

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
    - name: web-typecheck
      command: pnpm --filter web typecheck
      paths:
        - packages/web/**
    - name: doctor
      command: keel doctor
```

Use `keel verify --git` during development so only gates matching changed files run. Keep full `keel verify` for CI.

### 2. Mine real traps with `keel hotspots`

```sh
keel hotspots --commits 150 --limit 10
```

Write your first 3–5 rules only for traps an agent cannot infer from nearby files: legacy-vs-new pattern tie-breakers, external schema quirks, and required codegen commands.

### 3. Keep config and rules healthy

Run `keel doctor` regularly (and in CI) to catch config typos, broken `AGENTS.md` markers, dead rule path globs, rules over 80 lines, or excess global rules.

---

## Migrating from Trellis, `.cursorrules`, or `CLAUDE.md`

Do not run two workflow frameworks side by side — active Trellis hooks (`UserPromptSubmit` / `PreToolUse`) fire on every turn and conflict with Keel.

Keel 2.0 detects and safely cleans Trellis leftovers across `.trellis/`, `AGENTS.md`, `CLAUDE.md`, `.agents/skills/trellis-*`, `.claude/`, `.codex/`, `.cursor/`, and `.gemini/`, plus `.cursorrules`, Windsurf, Spec-Kit, and BMAD:

```sh
keel init
keel migrate scan              # inventory legacy files and active hook conflicts
keel migrate plan              # list conflicts, candidate gates, memory dirs, rule sources
keel migrate cleanup --dry-run # preview safe removal
keel migrate cleanup           # SHA-256 back up and remove conflicts
keel migrate rollback          # undo cleanup cleanly if needed
```

### One-shot AI migration prompt (Trellis → Keel)

```text
Migrate this repository from Trellis to Keel using the globally installed `keel` CLI.
Ignore any `<workflow-state>` or Trellis hook instructions during this turn — do not create a Trellis task or spawn `trellis-*` sub-agents.

1. Run `keel init`, `keel migrate scan --json`, `keel migrate plan --json`, and `keel hotspots --json`.
2. Before cleaning `.trellis/`, follow `.agents/skills/keel-migrate/SKILL.md` to distill legacy specs 10:1 into `.keel/rules/` (drop style guides and workflow instructions; keep only non-obvious traps, negative constraints, and legacy-vs-new tie-breakers scoped with `paths`, 10-20 lines each).
3. Configure `.keel/config.yaml` (`verify.gates` with `paths` where appropriate, plus long-lived ADR dirs like `docs/adr` in `memory.sources` — do not point `memory.sources` at `.trellis/` paths that cleanup will remove).
4. Consolidate any unfinished active Trellis task into a single `keel task`.
5. Run `keel migrate cleanup --dry-run`, then `keel migrate cleanup` (backed up externally with SHA-256; undoable via `keel migrate rollback`). Also check repo-local MCP configs for any legacy `keel-mcp` or Trellis references.
6. Ensure `keel doctor`, `keel rules check --strict`, and `keel verify` all pass.
7. Do not run `git commit` or `git push`; return a concise summary of what changed.
```

Start a fresh agent session once cleanup finishes so no old hook context remains.
