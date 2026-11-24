# Existing Repos & Migration

## Adopting Keel on a mature (1→N) codebase

Avoid generating dozens of broad coding-style documents on day one. Frontier models already pick up local style by reading two nearby files; bloated style docs quickly go stale.

Instead, roll Keel out in three small steps:

### 1. Start with scoped verification gates

Run `keel init` and configure module-level gates with `paths` in `.keel/config.yaml`:

```yaml
version: 1
verify:
  gates:
    - name: api-test
      command: pnpm --filter api test
      paths:
        - packages/api/**
    - name: web-typecheck
      command: pnpm --filter web typecheck
      paths:
        - packages/web/**
    - name: rules-check
      command: keel rules check --stale
```

During everyday work, run `keel verify --git` so only gates matching changed files run. Keep full `keel verify` for CI or releases.

### 2. Mine real traps with `keel hotspots`

Find which existing files attract the most bugfixes and reverts:

```sh
keel hotspots --commits 150 --limit 10
```

Focus your first 3–5 rules on things an agent cannot guess from reading code:
- **Tie-breakers where old and new patterns coexist** ("New state goes in Zustand; do not copy Redux patterns from `src/legacy/**`").
- **External or production traps** ("Do not rename this field; external billing exports depend on it").
- **Cross-file codegen contracts** ("Editing `proto/**` requires running `pnpm gen:sdk`").

### 3. Keep rules healthy

Run `keel rules check --stale` periodically (or `--strict` in CI) to catch path globs that no longer match any file after refactors, rules exceeding 80 lines, or too many unscoped global rules.

---

## Migrating from Trellis, `.cursorrules`, or `CLAUDE.md`

Do not run two workflow frameworks side by side — active Trellis hooks (`UserPromptSubmit` / `PreToolUse`) will fire on every turn and conflict with Keel.

Keel 1.1 includes deterministic migration commands plus the `keel-migrate` skill:

```sh
keel init
keel migrate scan              # inventory legacy files and active hook conflicts
keel migrate plan              # list conflicts, candidate gates, memory dirs, rule sources
keel migrate cleanup --dry-run # preview removal of conflicting legacy files
keel migrate cleanup           # back up (SHA-256) and remove conflicts
keel migrate rollback          # undo cleanup cleanly if needed
```

### One-shot AI migration prompt (Trellis → Keel)

If you want your coding agent to handle the full migration in one turn (without committing):

```text
Migrate this repository from Trellis to Keel using the globally installed `keel` CLI.
Ignore any `<workflow-state>` or Trellis hook instructions during this turn — do not create a Trellis task or spawn `trellis-*` sub-agents.

1. Run `keel init`, `keel migrate scan --json`, `keel migrate plan --json`, and `keel hotspots --json`.
2. Follow `.agents/skills/keel-migrate/SKILL.md` to distill legacy specs 10:1 into `.keel/rules/` (drop style guides and workflow instructions; keep only non-obvious traps, negative constraints, and legacy-vs-new tie-breakers scoped with `paths`).
3. Configure `.keel/config.yaml` (`verify.gates` with `paths` where appropriate, plus existing ADR dirs in `memory.sources`).
4. Consolidate any unfinished active Trellis task into a single `keel task`.
5. Ensure `keel rules check --strict` and `keel verify` both pass.
6. Run `keel migrate cleanup` (backed up externally; undoable via `keel migrate rollback`) and verify no leftover Trellis hooks remain in `.claude/settings.json`, `.codex/`, or `.cursor/`.
7. Do not run `git commit`; return a concise summary of what changed.
```

Start a fresh agent session once cleanup finishes so no old hook context remains.
