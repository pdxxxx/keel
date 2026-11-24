<div align="center">

<img src="./assets/keel.png" width="84" height="84" alt="keel" />

# keel

**Repo rules, task specs, session memory, and verification gates for coding agents.**

<p>
  <a href="https://www.npmjs.com/package/@pdxxxx/keel"><img src="https://img.shields.io/npm/v/%40pdxxxx%2Fkeel?style=flat-square&color=cb3837" alt="npm version" /></a>
  <a href="https://github.com/pdxxxx/keel/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/pdxxxx/keel/ci.yml?style=flat-square&label=CI" alt="CI" /></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-2563eb?style=flat-square" alt="MIT License" /></a>
  <img src="https://img.shields.io/badge/node-%3E%3D20-339933?style=flat-square&logo=node.js&logoColor=white" alt="Node >= 20" />
  <img src="https://img.shields.io/badge/rust-1.85%2B-dea584?style=flat-square&logo=rust&logoColor=black" alt="Rust 1.85+" />
</p>

<p>
  <a href="./README_CN.md">简体中文</a> ·
  <a href="./ARCHITECTURE.md">Architecture</a> ·
  <a href="./crates/keel-mcp/README.md">Rust MCP</a> ·
  <a href="./CHANGELOG.md">Changelog</a>
</p>

</div>

---

Keel stores project-specific rules, optional task specs, and check commands in plain files under `.keel/`, and searches local transcripts across Claude Code, Codex, OpenCode, and custom directories.

It does not manage sub-agents, install editor hooks, or lock you into a specific coding tool. Use it from the CLI or attach it over stdio MCP.

## Quick Start

Requires **Node.js >= 20**.

```sh
npm install -g @pdxxxx/keel@latest
```

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
keel mem search "refresh token"

# 5. Run checks matching your current git diff (or drop --git for all gates)
keel verify --git
```

## Existing Repos & Migration

On a mature codebase, start with checks and git history rather than writing broad style docs:

```sh
keel hotspots                         # find frequently fixed files worth adding rules for
keel rules check --stale              # spot dead path globs or bloated rules
keel migrate scan                     # inventory Trellis, .cursorrules, CLAUDE.md, etc.
keel migrate plan                     # list hook conflicts, candidate gates, and rule sources
keel migrate cleanup --dry-run        # preview safe removal (backed up; undo with `keel migrate rollback`)
```

Pair `keel migrate` with the bundled `keel-migrate` skill to distill only non-obvious traps into `.keel/rules/`.

## Commands

Every command supports `--json`.

| Command | Description |
| --- | --- |
| `keel init` | Creates `.keel/`, detects project scripts (`typecheck`, `lint`, `test`), updates `AGENTS.md`, and installs `.agents/skills/`. |
| `keel rules` | `list`, `match [paths...] [--git \| --staged \| --base <ref>]`, `add <id>`, `check [--stale \| --strict]`. |
| `keel task` | `create`, `list`, `show`, `done`, `reopen`, `archive` — optional single-file `spec.md` per task. |
| `keel verify` | Runs gates in `.keel/config.yaml` (`--gate`, `--git`, `--staged`, `--base <ref>`, `--fail-fast`). |
| `keel hotspots` | Ranks existing files by recent churn and bugfix commits (`--commits`, `--limit`). |
| `keel mem` | `sources`, `sessions`, `search <query>`, `show <id>` across local AI transcripts and `.keel/`. |
| `keel migrate` | `scan`, `plan`, `cleanup [--dry-run]`, `rollback` for moving off older AI setups safely. |
| `keel ablate` / `restore` | Temporarily removes Keel files into a SHA-256 verified snapshot for A/B testing, then restores them cleanly. |
| `keel mcp` | Starts the built-in stdio MCP server. |

## MCP Server

Two compatible stdio implementations are included: Node.js (`keel mcp`) and a standalone Rust binary (`crates/keel-mcp`, ~2 MiB, no Node runtime needed).

**Node.js**

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

**Native Rust** (`cargo build --release --locked --manifest-path crates/keel-mcp/Cargo.toml`)

```json
{
  "mcpServers": {
    "keel": {
      "command": "/path/to/keel/crates/keel-mcp/target/release/keel-mcp",
      "args": ["--root", "/path/to/your-project"]
    }
  }
}
```

<details>
<summary>Exposed MCP tools (9)</summary>

- `keel_rules_list` · `keel_rules_match` · `keel_rules_check` (`stale?: boolean`)
- `keel_task_list` · `keel_task_create` · `keel_task_show`
- `keel_verify` (`gates?: string[]`, `git?: boolean`, `failFast?: boolean`)
- `keel_mem_search` · `keel_mem_sessions`

</details>

See [crates/keel-mcp/README.md](./crates/keel-mcp/README.md) for Windows examples and parity checks.

## Repository Layout

```text
.keel/
  config.yaml        # verify gates (optional paths globs) and extra memory dirs
  rules/             # path-scoped markdown rules (YAML frontmatter)
  tasks/             # optional task specs (meta.json + spec.md)
AGENTS.md            # managed block (<!-- KEEL:START --> ... <!-- KEEL:END -->)
.agents/skills/      # keel-plan, keel-verify, keel-distill, keel-mem, keel-migrate
```

## License

[MIT](./LICENSE)
