# Getting Started

## Requirements

- **Node.js >= 20** for the CLI (`keel`) and built-in Node MCP server (`keel mcp`).
- **Rust 1.85+** (optional) only if you want to build the standalone `keel-mcp` binary from source. Prebuilt binaries are attached to [GitHub Releases](https://github.com/pdxxxx/keel/releases).

## Install

```sh
npm install -g @pdxxxx/keel@latest
```

Verify the installation:

```sh
keel --version
```

## Initialize a repository

Run `keel init` at the root of your project:

```sh
keel init
```

`keel init` writes only plain, git-friendly files:

```text
.keel/
  config.yaml        # detected verify gates (typecheck, lint, test) & memory sources
  rules/
    index.md         # rule format reference
  tasks/             # optional single-file task specs
AGENTS.md            # short managed block (<!-- KEEL:START --> ... <!-- KEEL:END -->)
.agents/skills/      # keel-plan, keel-verify, keel-distill, keel-mem, keel-migrate
```

If `AGENTS.md` already exists, Keel appends its managed block without modifying your existing content.

## Day-to-day usage

### Small edits

Just make the edit. Check `keel rules match --git` (or let your agent call `keel_rules_match` via MCP) to see if any invariant applies, then run `keel verify --git`. No task directory is needed.

### Multi-step or cross-session work

Create a single-file task spec to keep goals, non-goals, and acceptance criteria on disk across chat compactions:

```sh
keel task create "Refactor session refresh" --slug session-refresh
```

This creates `.keel/tasks/YYYY-MM-DD-session-refresh/spec.md` and `meta.json`. When finished:

```sh
keel task archive 2026-11-24-session-refresh
```

### Capturing lessons

When you fix a non-obvious bug or settle an architectural tradeoff:

1. If it can be checked by a test, linter, or script, add it to `.keel/config.yaml` under `verify.gates`.
2. Otherwise, add a concise scoped rule:

```sh
keel rules add no-float-money \
  --summary "Store monetary amounts as integer minor units" \
  --paths "src/payments/**"
```

Keep rules short and scoped with `paths` so agents only see them when touching relevant files.
