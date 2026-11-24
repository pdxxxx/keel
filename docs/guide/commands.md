# Commands Reference

Global flags available on every command:

- `-C, --root <PATH>` — target repository or any directory inside it
- `--json` — print machine-readable JSON

## `keel init`

```sh
keel init [-f | --force] [--reset-config] [--no-skills] [--no-detect]
```

Creates `.keel/`, auto-detects verification scripts (`package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`), upserts the `AGENTS.md` managed block (with strict paired-marker checks), and installs 5 skills (`keel-plan`, `keel-verify`, `keel-distill`, `keel-mem`, `keel-migrate`). Even with `--force`, an existing `.keel/config.yaml` is preserved unless `--reset-config` is passed.

## `keel update`

```sh
keel update [--dry-run] [--no-skills]
```

Refreshes only Keel-managed guidance files (`AGENTS.md` block, `.keel/rules/index.md`, `.agents/skills/keel-*/SKILL.md`). Never modifies `.keel/config.yaml`, user rules, or tasks.

## `keel doctor`

```sh
keel doctor
```

Validates `.keel/` structure, parses `.keel/config.yaml` strictly (reporting unknown keys, bad types, empty commands, and duplicate gate names), checks `AGENTS.md` marker pairing, confirms `git` is available on `PATH`, and runs `rules check --stale`. Exits non-zero on errors.

## `keel rules`

```sh
keel rules list
keel rules match [paths...] [--git] [--staged] [--base <ref>] [--no-global]
keel rules add <id> --summary <text> [--severity invariant|guideline|adr] [--paths <glob...>] [-f]
keel rules check [--stale] [--strict]
```

`check --stale` flags path globs matching zero files in the repo, rules over 80 lines, and more than 5 global rules (`--strict` exits non-zero on warnings).

## `keel task`

```sh
keel task create <title> [--slug <slug>]
keel task list [--all] [--status open|done]
keel task show <id>
keel task done <id>
keel task reopen <id>
keel task archive <id>
```

Archived tasks move to `.keel/tasks/archive/<id>/` and remain readable via `keel task show archive/<id>`.

## `keel verify`

```sh
keel verify [-g | --gate <name...>] [--git] [--staged] [--base <ref>] [--fail-fast]
```

Runs configured gates in order. Supports `paths` incremental filtering (`--git`/`--staged`/`--base`), per-gate `timeoutMs` (default 120s, terminates process tree on timeout with `exitCode: 124`), and `maxOutputChars` (default 16,000, keeps tail).

## `keel hotspots`

```sh
keel hotspots [--commits <n>] [--limit <n>]
```

Ranks existing files by recent commit churn and bugfix/revert frequency (`score = fixTouches * 3 + touches`), aligned cleanly even in nested monorepos.

## `keel mem`

```sh
keel mem sources [--source <name>]
keel mem sessions [--source <name>] [--scope project|all] [-n | --limit <count>]
keel mem search <query> [--source <name>] [--scope project|all] [-n | --limit <count>]
keel mem show <sessionId> [-n | --limit <count>]
```

Streaming read-only search across `keel`, `claude`, `codex`, `opencode`, `antigravity`, `gemini`, and `custom` directories.

## `keel migrate`

```sh
keel migrate scan
keel migrate plan
keel migrate cleanup [--dry-run] [--state-dir <dir>]
keel migrate rollback [--dry-run] [--force] [--state-dir <dir>]
```

Detects and safely retires conflicting legacy setups (Trellis across `.trellis/`, `AGENTS.md`, `CLAUDE.md`, `.agents/skills/trellis-*`, `.claude/`, `.codex/`, `.cursor/`, `.gemini/`; plus `.cursorrules`, Windsurf, Spec-Kit, BMAD) with SHA-256 external backup and conflict-safe rollback.

## `keel ablate` & `keel restore`

```sh
keel ablate [--dry-run] [--state-dir <dir>]
keel ablate status [--state-dir <dir>]
keel restore [--dry-run] [--force] [--state-dir <dir>]
```

Temporarily removes Keel surfaces into a SHA-256 verified external snapshot for A/B benchmarking and restores byte-for-byte.

## `keel mcp`

```sh
keel mcp [--root <path>] [--lock-root]
```

Starts the stdio JSON-RPC 2.0 MCP server exposing all 13 tools.
