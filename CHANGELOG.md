# Changelog

## 2.0.0 - 2026-11-24

Pure Rust single-binary release (`keel`).

### Architecture & Packaging
- Consolidated the entire CLI and MCP server into a single pure-Rust binary (`keel`, ~3.6 MiB); removed the duplicate TypeScript core, standalone `keel-mcp` crate, and cross-runtime parity harness.
- Replaced `@pdxxxx/keel` on npm with a zero-dependency thin launcher (~10 KB packed) that downloads and SHA-256 verifies the matching GitHub Release binary on install.
- Updated CI and Release workflows for five native binary targets (`linux-x86_64`, `linux-aarch64`, `macos-arm64`, `macos-x86_64`, `windows-x86_64.exe`), uploading GitHub Release assets and `SHA256SUMS.txt` before publishing to npm via OIDC Trusted Publishing.

### New Commands & Reliability Fixes
- **`keel update`**: safely refreshes managed templates (`AGENTS.md` block, `.keel/rules/index.md`, `.agents/skills/keel-*`) without touching `.keel/config.yaml`, user rules, or tasks. `keel init --force` no longer overwrites user `config.yaml` unless `--reset-config` is passed.
- **`keel doctor`**: validates `.keel/config.yaml` strictly (catching typos, bad types, duplicate gate names), `AGENTS.md` marker integrity, `git` availability, and stale rules.
- **Global `-C, --root <PATH>` & `--json`**: supported consistently across CLI commands with clean single-line `stderr` error reporting.
- **Multi-repo MCP & 13 tools**: all 13 MCP tools (`keel_rules_*`, `keel_task_*`, `keel_verify`, `keel_hotspots`, `keel_mem_*`) accept highest-priority per-call `root`, support `initialize` `rootUri`/`workspaceFolders`, enforce homeless-directory write protection, and support `--lock-root`.
- **Verification safety**: added `timeoutMs` (default 120s, process-tree termination) and `maxOutputChars` (default 16,000, tail-preserving truncation) to `verify.gates`.
- **Memory & Git improvements**: added `--scope project | all` filtering and current-project score boosting, read-only `antigravity` and `gemini` transcript adapters, `keel_mem_show`, and nested monorepo git-path alignment.
- **Migration coverage**: expanded `keel migrate` to detect and safely clean Trellis leftovers in `.agents/skills/trellis-*`, `.codex/`, `.cursor/hooks*`, `.gemini/settings.json`, and `CLAUDE.md`.

## 1.1.0 - 2026-11-24

- Added incremental `verify.gates[].paths` + `keel verify --git / --staged / --base`, `keel rules check --stale / --strict`, `keel hotspots`, and `keel migrate scan / plan / cleanup / rollback`.

## 1.0.0 - 2026-11-24

- Initial release (`init`, `rules`, `task`, `verify`, `mem`, `ablate` / `restore`, `mcp`).
