# v1.2: Multi-Repo MCP Root Resolution (Aggregator Support)

## Goal

Make a single long-lived `keel mcp` or `keel-mcp` process safely serve multiple repositories concurrently (e.g. behind MCP hubs/aggregators like Magpie, MetaMCP, OneMCP, or global Codex/Desktop configs), with zero cross-repo leakage and full backward compatibility.

## Non-Goals

- Do not break existing callers that omit `arguments.root` when running with `--root` or inside a repository `cwd`.
- Do not cache per-repo state across requests (keep every `tools/call` stateless per resolved root).
- Do not add vendor-specific hub code.

## Specification

### 1. Per-Call `root` Parameter on All 9 MCP Tools

Add an optional `root` property to `inputSchema.properties` on **all 9 MCP tools** (`keel_rules_list`, `keel_rules_match`, `keel_rules_check`, `keel_task_list`, `keel_task_create`, `keel_task_show`, `keel_verify`, `keel_mem_search`, `keel_mem_sessions`) in both `src/core/mcp.ts` and `crates/keel-mcp/src/mcp.rs`:

```json
"root": {
  "type": "string",
  "description": "Absolute path to the target repository (or any directory inside it). Always pass this whenever known so a shared or aggregated MCP server targets the right repository."
}
```

- Keep `required` arrays unchanged (backward-compatible).
- Both Node and Rust schemas MUST match identically (verified in `parity.mjs`).

### 2. Deterministic Root Resolution Precedence (Per Request)

Resolve the target repository root for each `tools/call` in strict priority order:

1. **`arguments.root` (Highest priority)**
   - If provided (non-empty trimmed string):
     - Resolve relative paths against server fallback root/cwd (or require existing directory).
     - Verify the path exists and is a directory; if not, return a tool error (`isError: true`): `Invalid root directory: <raw>`.
     - Walk upward from that directory to find the nearest ancestor containing `.keel/`, then nearest containing `.git`, otherwise use that directory itself.
2. **Startup `--root <path>`**
   - Available on both `keel-mcp [--root <path>] [--lock-root]` and `keel mcp [--root <path>] [--lock-root]`.
3. **MCP `initialize` workspace root (when `--root` was NOT passed)**
   - During `initialize`, inspect `params.rootUri` (string `file://...`) or `params.workspaceFolders[0].uri` (string `file://...`).
   - Decode valid `file://` URIs cleanly on both Unix (`file:///home/dev/repo`) and Windows (`file:///D:/Workspace/keel` or `file:///d%3A/Workspace/keel`).
   - If the decoded path exists and is a directory, resolve upward (`.keel/` -> `.git/` -> path) and remember it as the session default root (still overridden by any per-call `arguments.root`).
4. **Process `cwd` upward discovery** (fallback when none of the above were provided).

### 3. Homeless / Aggregator Safety Guard

When **none** of `arguments.root`, startup `--root`, or `initialize` `rootUri`/`workspaceFolders` were provided, AND upward discovery from process `cwd` found **neither** `.keel/` **nor** `.git/`:
- The server is in an ambiguous/homeless directory (typical for MCP aggregators launched from `~` or their install folder).
- For `keel_task_create` and `keel_verify`, DO NOT silently create `.keel/tasks/` in the homeless directory or falsely report `passed: true` with 0 gates; instead return a tool error (`isError: true`):
  `No .keel/ or .git/ repository found from working directory; pass "root" pointing to the target repository.`
- For read-only tools (`keel_rules_*`, `keel_task_list`, `keel_task_show`, `keel_mem_*`), keep existing read-only behavior (returning empty lists / external memory hits).
- When `--root` or `arguments.root` IS explicitly passed, trust that directory even if `.keel/` / `.git/` do not exist yet (e.g. brand-new repo or test tempdir).

### 4. Optional `--lock-root` Sandbox Flag

- Support `--lock-root` on both `keel mcp [--root <path>] [--lock-root]` and `keel-mcp [--root <path>] [--lock-root]`.
- When `--lock-root` is enabled, any `arguments.root` (after resolving realpath/canonical path) that lies outside the locked root directory MUST be rejected with a tool error (`isError: true`):
  `Root path escapes locked root: <raw>`
- Without `--lock-root`, `arguments.root` freely targets any valid directory (highest priority).

### 5. `keel mcp` CLI Parity & Version Bump

- Add `--root <path>` and `--lock-root` options to `keel mcp` in `src/cli.ts` (matching `keel-mcp`).
- Update `renderAgentsBlock()` in `src/templates/assets.ts` to add one concise note reminding agents to pass `root` when calling `keel_*` MCP tools.
- Bump version from `1.1.0` to `1.2.0` in `package.json`, `src/version.ts`, and `crates/keel-mcp/Cargo.toml` (+ `Cargo.lock`).

## Acceptance Criteria

- [ ] All 9 MCP tools expose the exact same `root` property and description in both TS and Rust.
- [ ] A single running `keel mcp` / `keel-mcp` process serves two distinct repos (Repo A and Repo B) via `arguments.root` (including subdirectories inside Repo A/B) with zero cross-repo leakage across `rules`, `task`, `verify`, and `mem`.
- [ ] Non-existent or file-path `arguments.root` returns `isError: true` (`Invalid root directory: ...`).
- [ ] `initialize` `rootUri` / `workspaceFolders` works on both POSIX and Windows `file://` URIs when `--root` is omitted, and is still overridden by `arguments.root`.
- [ ] Homeless `cwd` (no `--root`, no `rootUri`, no `arguments.root`, no `.keel`/`.git`) rejects `keel_task_create` and `keel_verify` with the exact guard error without creating `.keel/` on disk.
- [ ] `--lock-root` rejects `arguments.root` outside the locked root while allowing paths inside it.
- [ ] `node bin/keel.js verify`, `cargo test --locked`, `cargo clippy --all-targets -- -D warnings`, `cargo build --release --locked`, and `node crates/keel-mcp/tests/parity.mjs` (expanded with multi-repo `arguments.root`, `rootUri`, homeless guard, and `--lock-root` cases) all pass.
- [ ] Do NOT edit `README.md`, `README_CN.md`, `ARCHITECTURE.md`, `CHANGELOG.md`, `crates/keel-mcp/README.md`, or `docs/**` (documentation will be updated separately).
- [ ] Do NOT run `git commit` or `git push`.
