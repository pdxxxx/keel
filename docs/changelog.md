# Changelog

## 2.0.1 — 2026-11-24

- **Windows executable icon & metadata**: embedded `assets/keel.ico` (16–256px) and PE version metadata into `keel.exe` via `build.rs`.
- **Copy-ready agent prompts**: added onboarding and upgrade prompts to Getting Started and tightened the Trellis → Keel migration prompt.
- **Release tooling**: removed the hardcoded `2.0.0` version checks in `.github/check-release.mjs` and MCP handshake tests.

## 2.0.0 — 2026-11-24

Pure Rust single-binary release (`keel`).

- **Single pure-Rust binary**: consolidated the entire CLI and 13-tool stdio MCP server into one ~3.6 MiB executable; removed the duplicate TypeScript core, standalone `keel-mcp` crate, and dual-stack parity harness.
- **Zero-dependency npm distribution**: `@pdxxxx/keel` now ships a ~10 KB launcher that downloads and SHA-256 verifies the matching GitHub Release binary across 5 targets (`linux-x86_64`, `linux-aarch64`, `macos-arm64`, `macos-x86_64`, `windows-x86_64.exe`), published via OIDC Trusted Publishing after Release assets are live.
- **Safe upgrades & diagnostics**: added `keel update` (refreshes managed templates without touching `.keel/config.yaml`, rules, or tasks), `keel doctor` (strict config/marker/git/rules health check), global `-C/--root`, and clean single-line `stderr` diagnostics.
- **Multi-repo MCP & 13 tools**: added highest-priority per-call `root` across all 13 tools, `initialize` `rootUri`/`workspaceFolders`, homeless-directory write guard, `--lock-root`, plus `keel_rules_add`, `keel_task_update`, `keel_hotspots`, and `keel_mem_show`.
- **Verification safety**: added `timeoutMs` (process-tree termination on timeout) and `maxOutputChars` (tail-preserving truncation) to `verify.gates`.
- **Memory, Git & Migration**: added `--scope project | all` filtering and current-repo boosting, `antigravity` & `gemini` transcript adapters, nested monorepo git-path alignment, and multi-platform Trellis cleanup (`.agents/skills/trellis-*`, `.codex/`, `.cursor/hooks*`, `.gemini/settings.json`, `CLAUDE.md`).

## 1.1.0 — 2026-11-24

- Added incremental `verify.gates[].paths` + `keel verify --git / --staged / --base`, `keel rules check --stale / --strict`, `keel hotspots`, and `keel migrate scan / plan / cleanup / rollback`.

## 1.0.0 — 2026-11-24

- Initial release (`init`, `rules`, `task`, `verify`, `mem`, `ablate` / `restore`, `mcp`).
