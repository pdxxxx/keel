# Changelog

## 1.0.0 - 2026-11-24

Initial release.

- `keel init`: creates `.keel/`, auto-detects project verification scripts, writes a managed block into `AGENTS.md`, and installs four skill files (`keel-plan`, `keel-verify`, `keel-distill`, `keel-mem`).
- `keel rules`: `list`, `match` (path globs plus `--git`, `--staged`, `--base`), `add`, and `check`.
- `keel task`: `create`, `list`, `show`, `done`, `reopen`, and `archive`.
- `keel verify`: runs configured commands from `.keel/config.yaml` with `--gate`, `--fail-fast`, and `--json`.
- `keel mem`: `sources`, `sessions`, `search`, and `show` across `.keel/` files and local Claude Code, Codex, OpenCode, and custom JSONL transcripts.
- `keel ablate` / `keel restore`: SHA-256 verified temporary removal and restoration of Keel files with conflict checks.
- `keel mcp` & `crates/keel-mcp`: stdio MCP server in both Node.js and Rust, verified against each other with `crates/keel-mcp/tests/parity.mjs`.
