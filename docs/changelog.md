# Changelog

## 1.1.0 — 2026-11-24

Brownfield (1→N) and migration release.

- **Incremental verification**: added optional `paths` globs on `verify.gates` plus `keel verify --git / --staged / --base <ref>` (and `git: true` on MCP `keel_verify` in both Node and Rust).
- **Rule staleness & hygiene checks**: added `keel rules check --stale` / `--strict` (and `stale: true` on MCP `keel_rules_check`) to flag dead path globs, rules over 80 lines, and excess global rules while preserving the 1.0.0 default output.
- **Git bugfix hotspots**: added `keel hotspots` to rank existing files by recent churn and bugfix commits.
- **One-shot migration tools**: added `keel migrate scan / plan / cleanup / rollback` backed by shared external SHA-256 snapshots, plus the `keel-migrate` standard skill installed by `keel init`.
- **Automated publishing**: added `.github/workflows/release.yml` for npm OIDC Trusted Publishing (`@pdxxxx/keel`) and three-platform Rust `keel-mcp` GitHub Release binaries.

## 1.0.0 — 2026-11-24

Initial release.

- `keel init`: creates `.keel/`, auto-detects project verification scripts, writes a managed block into `AGENTS.md`, and installs standard skills.
- `keel rules`: `list`, `match` (path globs plus `--git`, `--staged`, `--base`), `add`, and `check`.
- `keel task`: `create`, `list`, `show`, `done`, `reopen`, and `archive`.
- `keel verify`: runs configured commands from `.keel/config.yaml` with `--gate`, `--fail-fast`, and `--json`.
- `keel mem`: `sources`, `sessions`, `search`, and `show` across `.keel/` files and local Claude Code, Codex, OpenCode, and custom JSONL transcripts.
- `keel ablate` / `keel restore`: SHA-256 verified temporary removal and restoration of Keel files with conflict checks.
- `keel mcp` & `crates/keel-mcp`: stdio MCP server in both Node.js and Rust, verified against each other with `crates/keel-mcp/tests/parity.mjs`.
