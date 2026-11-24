<div align="center">

<img src="./assets/keel.png" width="84" height="84" alt="keel" />

# keel

**Repo rules, task specs, session memory, and verification gates for coding agents.**

<p>
  <a href="https://www.npmjs.com/package/@pdxxxx/keel"><img src="https://img.shields.io/npm/v/%40pdxxxx%2Fkeel?style=flat-square&color=cb3837" alt="npm version" /></a>
  <a href="https://github.com/pdxxxx/keel/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/pdxxxx/keel/ci.yml?style=flat-square&label=CI" alt="CI" /></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-2563eb?style=flat-square" alt="MIT License" /></a>
  <img src="https://img.shields.io/badge/rust-1.85%2B-dea584?style=flat-square&logo=rust&logoColor=black" alt="Rust 1.85+" />
</p>

<p>
  <a href="https://pdxxxx.github.io/keel/">Docs</a> ·
  <a href="./README_CN.md">简体中文</a> ·
  <a href="./ARCHITECTURE.md">Architecture</a> ·
  <a href="./CHANGELOG.md">Changelog</a>
</p>

</div>

---

Keel is a single Rust binary (~3.6 MiB) that stores project rules, optional task specs, and check commands under `.keel/`, searches local session logs across seven tools, and exposes 13 tools over stdio MCP.

It does not manage sub-agents, install editor hooks, or tie you to a specific coding tool.

## Install

```sh
# Recommended: installs the prebuilt native binary verified with SHA-256
npm install -g @pdxxxx/keel@latest
```

Or build directly from source with Rust 1.85+:

```sh
cargo install --git https://github.com/pdxxxx/keel --locked
```

Prebuilt binaries for Linux (`x86_64`, `aarch64`), macOS (`arm64`, `x86_64`), and Windows (`x86_64`) are also attached to [GitHub Releases](https://github.com/pdxxxx/keel/releases).

## Quick Start

```sh
# 1. Initialize .keel/, AGENTS.md block, and standard .agents/skills/
keel init

# 2. Record a path-scoped rule
keel rules add auth-rotation \
  --summary "Rotate refresh tokens atomically" \
  --paths "src/auth/**"

# 3. Pull rules matching your current git diff
keel rules match --git

# 4. Search past sessions and .keel/ docs
keel mem search "refresh token" --scope project

# 5. Run checks matching your current git diff (drop --git for all gates)
keel verify --git
```

## Existing Repos & Migration

```sh
keel hotspots                    # rank existing files by recent bugfix churn
keel doctor                      # check config.yaml, AGENTS.md markers, git, and stale rules
keel update                      # refresh AGENTS.md block & skills without touching config.yaml
keel migrate scan                # inventory Trellis, .cursorrules, CLAUDE.md, etc.
keel migrate cleanup --dry-run   # preview safe removal (backed up; undo with `keel migrate rollback`)
```

## Commands

Global flags: `-C, --root <PATH>` and `--json`.

| Command | Description |
| --- | --- |
| `keel init` | Creates `.keel/`, detects project scripts, updates `AGENTS.md`, installs 5 skills (never overwrites `config.yaml` unless `--reset-config`). |
| `keel update` | Safely refreshes managed templates (`AGENTS.md` block, `rules/index.md`, `keel-*` skills) without touching user config, rules, or tasks. |
| `keel doctor` | Validates `.keel/config.yaml` strictly, checks `AGENTS.md` markers, `git`, and stale rules. |
| `keel rules` | `list`, `match [paths...] [--git \| --staged \| --base <ref>]`, `add <id>`, `check [--stale \| --strict]`. |
| `keel task` | `create`, `list`, `show`, `done`, `reopen`, `archive` — optional single-file `spec.md` per task. |
| `keel verify` | Runs gates in `.keel/config.yaml` (`--gate`, `--git`, `--staged`, `--base <ref>`, `--fail-fast`) with timeout and output-cap protection. |
| `keel hotspots` | Ranks existing files by recent churn and bugfix commits (`--commits`, `--limit`). |
| `keel mem` | `sources`, `sessions`, `search`, `show` (`--scope project \| all`) across `.keel/`, Claude Code, Codex, OpenCode, Antigravity, Gemini, and custom dirs. |
| `keel migrate` | `scan`, `plan`, `cleanup [--dry-run]`, `rollback` backed by SHA-256 external snapshots. |
| `keel ablate` / `restore` | Temporarily removes Keel surfaces for A/B benchmarking and restores byte-for-byte. |
| `keel mcp` | Starts the stdio MCP server (13 tools, multi-repo per-call `root`, optional `--lock-root`). |

## MCP Server

A single `keel mcp` process can serve multiple repositories safely (including behind MCP hubs/aggregators). Every tool accepts an optional `root` argument as highest priority.

**Codex CLI (global)**

```sh
codex mcp add keel -- keel mcp
```

**Standard `mcpServers` JSON** (Claude Code, Cursor, OpenCode, Antigravity)

```json
{
  "mcpServers": {
    "keel": {
      "command": "keel",
      "args": ["mcp"]
    }
  }
}
```

Pass `--root <path> --lock-root` if you want to pin an instance to a single repository.

<details>
<summary>Exposed MCP tools (13)</summary>

- `keel_rules_list` · `keel_rules_match` · `keel_rules_add` · `keel_rules_check`
- `keel_task_list` · `keel_task_create` · `keel_task_show` · `keel_task_update`
- `keel_verify` · `keel_hotspots`
- `keel_mem_search` · `keel_mem_sessions` · `keel_mem_show`

</details>

## Repository Layout

```text
.keel/
  config.yaml        # verify gates (paths, timeoutMs, maxOutputChars) & memory sources
  rules/             # path-scoped markdown rules (YAML frontmatter)
  tasks/             # optional task specs (meta.json + spec.md)
AGENTS.md            # managed block (<!-- KEEL:START --> ... <!-- KEEL:END -->)
.agents/skills/      # keel-plan, keel-verify, keel-distill, keel-mem, keel-migrate
```

## License

[MIT](./LICENSE)
