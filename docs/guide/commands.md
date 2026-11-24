# Commands Reference

Every command supports `--json` for machine-readable output.

## `keel init`

Initialize Keel in the current directory.

```sh
keel init [-f | --force] [--no-skills] [--no-detect] [--json]
```

- Auto-detects common verification scripts from `package.json`, `Cargo.toml`, `go.mod`, or `pyproject.toml`.
- Upserts a short managed block in `AGENTS.md` between `<!-- KEEL:START -->` and `<!-- KEEL:END -->`.
- Installs five harness-agnostic skills under `.agents/skills/`: `keel-plan`, `keel-verify`, `keel-distill`, `keel-mem`, `keel-migrate`.

## `keel rules`

Manage path-scoped Markdown rules under `.keel/rules/`.

```sh
keel rules list [--json]
keel rules match [paths...] [--git] [--staged] [--base <ref>] [--no-global] [--json]
keel rules add <id> --summary <text> [--severity invariant|guideline|adr] [--paths <glob...>] [-f] [--json]
keel rules check [--stale] [--strict] [--json]
```

- `match` combines explicit paths with changed files from git when `--git`, `--staged`, or `--base <ref>` is set.
- `check --stale` flags path globs matching zero files in the repository, rules over 80 lines, and repositories with more than 5 global rules. `--strict` exits non-zero on warnings as well as schema errors.

## `keel task`

Optional lightweight task specs under `.keel/tasks/`.

```sh
keel task create <title> [--slug <slug>] [--json]
keel task list [--all] [--status open|done] [--json]
keel task show <id> [--json]
keel task done <id> [--json]
keel task reopen <id> [--json]
keel task archive <id> [--json]
```

Each task contains a `meta.json` and a single living `spec.md` (plus an optional `research/` directory for notes). Archived tasks move to `.keel/tasks/archive/<id>/` and can still be read via `keel task show archive/<id>`.

## `keel verify`

Run deterministic verification gates defined in `.keel/config.yaml`.

```sh
keel verify [-g | --gate <name...>] [--git] [--staged] [--base <ref>] [--fail-fast] [--json]
```

- Without flags, runs every configured gate in order.
- With `--git`, `--staged`, or `--base <ref>` (and no explicit `--gate`), runs global gates (no `paths`) plus gates whose `paths` globs match changed files.
- Exits non-zero if any executed gate fails.

## `keel hotspots`

Rank existing repository files by recent commit churn and bugfix/revert frequency.

```sh
keel hotspots [--commits <n>] [--limit <n>] [--json]
```

Defaults to inspecting the last 100 commits and returning the top 10 existing files (`score = fixTouches * 3 + touches`), skipping deleted files.

## `keel mem`

Read-only search across `.keel/` files and local AI session transcripts (Claude Code, Codex, OpenCode, and custom directories configured in `.keel/config.yaml` or `KEEL_MEM_DIRS`).

```sh
keel mem sources [--source <name>] [--json]
keel mem sessions [--source <name>] [-n | --limit <count>] [--json]
keel mem search <query> [--source <name>] [-n | --limit <count>] [--json]
keel mem show <sessionId> [-n | --limit <count>] [--json]
```

## `keel migrate`

Inspect and safely retire conflicting legacy AI setups (Trellis, `.cursorrules`, `CLAUDE.md`, Copilot instructions, Windsurf, Spec-Kit, BMAD).

```sh
keel migrate scan [--json]
keel migrate plan [--json]
keel migrate cleanup [--dry-run] [--state-dir <dir>] [--json]
keel migrate rollback [--dry-run] [--force] [--state-dir <dir>] [--json]
```

`cleanup` writes a SHA-256 verified external backup before modifying the repository. `rollback` checks all paths for conflicts before restoring.

## `keel ablate` & `keel restore`

Temporarily remove all Keel surfaces (`.keel/`, `.agents/skills/keel-*`, and the managed `AGENTS.md` block) into a verified external snapshot for A/B testing, then restore byte-for-byte.

```sh
keel ablate [--dry-run] [--state-dir <dir>] [--json]
keel ablate status [--state-dir <dir>] [--json]
keel restore [--dry-run] [--force] [--state-dir <dir>] [--json]
```

## `keel mcp`

Start the built-in Node.js stdio MCP server for the current repository.

```sh
keel mcp
```
