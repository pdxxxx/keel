# v1.1: Brownfield (1->N) & Migration Tools

## Goal

Make Keel practical on large existing repositories and projects migrating from older AI coding frameworks (Trellis, Cursor rules, CLAUDE.md, Spec-Kit, BMAD), while keeping zero LLM API dependencies and strict TS/Rust MCP parity.

## Non-Goals

- Do not call external LLM APIs inside the CLI or Rust binary.
- Do not copy raw legacy Markdown specs blindly into `.keel/rules/`.
- Do not add vendor hooks or sub-agent orchestration rules.
- Do not break existing 1.0.0 `.keel/config.yaml`, `.keel/rules/`, `.keel/tasks/`, or MCP tool schemas.

## Feature Specifications

### 1. Scoped Incremental Verification (`verify.gates[].paths` + `--git`)

- Extend `.keel/config.yaml` gate schema with optional `paths?: string[]` (same POSIX glob syntax as `.keel/rules/`):
  ```yaml
  verify:
    gates:
      - name: api-test
        command: pnpm --filter api test
        paths:
          - packages/api/**
  ```
- Behavior:
  - `keel verify` (default): runs all configured gates (unchanged from 1.0.0).
  - `keel verify --git` / `--staged` / `--base <ref>`: resolves changed repository files via git; runs every gate whose `paths` matches at least one changed file, PLUS global gates (gates with no `paths` or empty `paths`).
  - If `--gate <name...>` is passed alongside `--git`, explicit `--gate` names take precedence over path filtering (or filter first, document and match between TS and Rust — specifically: when `--gate` is explicitly provided, run those exact named gates; when `--gate` is omitted and `--git`/`--staged`/`--base` is set, filter by changed paths).
  - When `--git` resolves 0 changed files, only global gates (no `paths`) run; if no gates match, return `{ passed: true, gates: [], skipped: [...] }` (or keep `passed: true, gates: []` and add optional `skipped?: string[]` listing gate names skipped due to `paths` filter).
- MCP parity (`src/core/mcp.ts` & `crates/keel-mcp`):
  - Extend `keel_verify` inputSchema with optional `git?: boolean` (when `true` and `gates` is omitted/empty, filter by uncommitted git changes using the same rule as CLI).
  - Update Rust `src/config.rs` and `src/verify.rs` to parse `paths` and support `git: true`.

### 2. Rule Staleness & Hygiene Check (`keel rules check --stale`)

- Default `keel rules check` stays 100% backward-compatible with 1.0.0.
- Add `--stale` flag to `keel rules check` (and optional `stale?: boolean` parameter to MCP `keel_rules_check`):
  - Check every non-global rule's `paths` globs against actual tracked/working-tree files in the repository (skipping `.git`, `node_modules`, `target`, `dist`).
  - If a `paths` pattern matches 0 files in the repo, report a warning/issue under `warnings: RuleIssue[]` (or report in `warnings` array so `valid` still reflects schema errors unless `--strict` is also passed!).
  - Also emit hygiene warnings in `warnings` when:
    - A single rule file exceeds 80 lines (encourage splitting or moving to executable gates).
    - More than 5 global rules (no `paths`) exist in `.keel/rules/`.
  - Add `--strict` flag on CLI: exit non-zero if `warnings.length > 0` in addition to schema `issues`.
- Keep TS and Rust `keel_rules_check` JSON output identical when `stale` is omitted (1.0.0 format) and when `stale: true` (adds `warnings: [...]`).

### 3. Git Bugfix Hotspot Analyzer (`keel hotspots`)

- New CLI command: `keel hotspots [--commits <n>] [--limit <n>] [--json]`
  - Default `--commits 100` (inspect last 100 commits via `git log`), default `--limit 10`.
  - Scans commit subjects + changed file lists from `git log -n <commits> --name-only --pretty=format:---COMMIT---%n%H%n%s` (no external deps).
  - Scores files by:
    - `touches`: total commits touching the file (only files that still exist on disk).
    - `fixTouches`: commits whose subject starts with or contains bugfix/revert markers (`fix`, `bug`, `revert`, `hotfix`, `patch`, case-insensitive word boundary).
    - `score`: `fixTouches * 3 + touches`.
  - Returns top `limit` existing files sorted by `score` desc, with `file`, `score`, `touches`, `fixTouches`, and up to 3 recent `recentFixes` (commit hash prefix + subject).
- Purpose: lets an engineer or agent find the highest-risk areas of a legacy codebase in seconds instead of reading the whole repository.

### 4. One-Shot Migration CLI (`keel migrate`) + `keel-migrate` Skill

- New CLI subcommand group `keel migrate`:
  - `keel migrate scan [--json]`
    - Read-only discovery of legacy AI framework surfaces in the repo:
      - `trellis`: `.trellis/`, `AGENTS.md` containing `<!-- TRELLIS:START -->`, `.claude/agents/trellis-*`, `.claude/hooks/*` or `.claude/settings.json` referencing `trellis`, `.cursor/commands/trellis*` or `.cursor/agents/trellis-*`.
      - `cursor`: `.cursorrules`, `.cursor/rules/*.mdc` (also extracts `globs` from frontmatter if present).
      - `claude`: `CLAUDE.md`.
      - `copilot`: `.github/copilot-instructions.md`.
      - `windsurf`: `.windsurfrules`, `.windsurf/rules/**`.
      - `spec-kit`: `.specify/`.
      - `bmad`: `_bmad/`.
    - Classifies each found item into:
      - `category`: `"conflict"` (active hooks / competing workflow blocks / custom agent dispatchers), `"rule-source"` (candidate rules to distill), `"memory-source"` (archives / ADRs / design docs like `docs/adr`, `.trellis/tasks/archive`), or `"gate-source"` (`package.json`, `.github/workflows/*.yml`, `Makefile`).
  - `keel migrate plan [--json]`
    - Runs `scan` and produces a structured migration plan:
      - `conflicts`: high-priority items to disable/remove so they do not fight Keel.
      - `suggestedGates`: detected verification gates (reusing `detectProjectGates`).
      - `suggestedMemorySources`: existing archive/ADR directories to add to `.keel/config.yaml` `memory.sources`.
      - `ruleSources`: list of legacy rule files with any extracted `globs` hints, for the agent/human to distill 10:1 via `keel-migrate` skill.
  - `keel migrate cleanup [--dry-run] [--state-dir <dir>] [--json]`
    - Safely removes/strips detected `"conflict"` surfaces (e.g. strips `<!-- TRELLIS:START --> ... <!-- TRELLIS:END -->` from `AGENTS.md` while keeping Keel block & user prose; removes `.trellis/`, `.cursorrules`, conflicting hook/agent files).
    - **Safety contract**: MUST back up every mutated/deleted file with SHA-256 verification into an external snapshot (`~/.keel-migrate/<repo-hash>/` or `--state-dir`) BEFORE mutating the repo, and support `keel migrate rollback [--state-dir <dir>] [--force] [--json]` using the exact same conflict-safe restore contract as `ablate.ts`!
    - Note: if `.trellis/` exists, cleanup backs it up and removes it cleanly; `--dry-run` previews every entry without touching disk.
- Standard skill `keel-migrate`:
  - Embed the exact text of `.agents/skills/keel-migrate/SKILL.md` into `src/templates/assets.ts` so `keel init` installs it alongside the existing 4 skills.

### 5. Version Bump & Parity

- Bump version from `1.0.0` to `1.1.0` in `package.json`, `src/version.ts`, and `crates/keel-mcp/Cargo.toml` (+ `Cargo.lock`).
- Update `crates/keel-mcp/tests/parity.mjs` to test:
  - `keel_verify` with scoped `paths` and `git: true`
  - `keel_rules_check` with `stale: true` (both clean and orphan-glob cases)
  - Version `1.1.0` handshake

## Acceptance Criteria

- [ ] `keel verify --git` skips gates whose `paths` do not match changed files and still runs global gates; `keel_verify` with `git: true` matches in both TS and Rust.
- [ ] `keel rules check --stale` detects orphan `paths` matching zero repo files and oversized/excess global rule warnings; `keel_rules_check` with `stale: true` matches in both TS and Rust.
- [ ] `keel hotspots` returns ranked bugfix hotspots from git history and handles non-git repos gracefully.
- [ ] `keel migrate scan`, `plan`, `cleanup` (with `--dry-run`), and `rollback` work end-to-end with SHA-256 backup verification and conflict-safe rollback.
- [ ] `keel init` installs `keel-migrate` skill matching `.agents/skills/keel-migrate/SKILL.md`.
- [ ] `node bin/keel.js verify`, `cargo test --locked`, `cargo clippy --all-targets -- -D warnings`, `cargo build --release --locked`, and `node crates/keel-mcp/tests/parity.mjs` all pass.
- [ ] Do NOT edit `README.md`, `README_CN.md`, `ARCHITECTURE.md`, or `CHANGELOG.md` (documentation updates will be handled separately).
- [ ] Do NOT run `git commit` or `git push`.
