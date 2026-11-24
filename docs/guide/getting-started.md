# Getting Started

## Install

Install via npm (downloads and SHA-256 verifies the prebuilt native Rust binary for your platform):

```sh
npm install -g @pdxxxx/keel@latest
```

Or build from source with Rust 1.89+:

```sh
cargo install --git https://github.com/pdxxxx/keel --locked
```

Prebuilt single binaries for Linux (`x86_64`, `aarch64`), macOS (`arm64`, `x86_64`), and Windows (`x86_64`) are also available on [GitHub Releases](https://github.com/pdxxxx/keel/releases).

Verify:

```sh
keel --version
```

## Initialize a repository

```sh
keel init
```

This creates:

```text
.keel/
  config.yaml        # detected verify gates (typecheck, lint, test) & memory sources
  rules/
    index.md         # rule format reference
  tasks/             # optional single-file task specs
AGENTS.md            # managed block (<!-- KEEL:START --> ... <!-- KEEL:END -->)
.agents/skills/      # keel-plan, keel-verify, keel-distill, keel-mem, keel-migrate
```

`keel init` preserves user text outside the managed `AGENTS.md` markers and never overwrites an existing `.keel/config.yaml` unless you explicitly pass `--reset-config`.

### Prompt for your coding agent (new repository)

```text
Initialize Keel in this repository using the installed `keel` CLI (verify `keel --version` is 2.x). Complete all steps without pausing, and do not run `git commit` or `git push`:

1. Run `keel init`.
2. Review `.keel/config.yaml`: replace or add real `verify.gates` commands based on the repo manifests and CI workflows (add `paths` on module gates in monorepos). If this is a brand-new empty repo with no build scripts yet, leave `verify.gates: []` instead of inventing fake commands. Add any existing ADR directory (e.g. `docs/adr`) to `memory.sources`.
3. Only add `.keel/rules/` entries via `keel rules add --paths` for non-obvious traps, negative constraints, or external quirks that cannot be enforced by a gate or inferred from nearby code (10-20 lines each). Do not write generic style guides or directory summaries; zero rules is fine for a fresh repo.
4. Run `keel doctor`, `keel rules check --strict`, and (if gates exist) `keel verify`.
5. Summarize what was initialized, the configured gates, any rules added, and verification results.
```

## Upgrading existing projects

After upgrading the `keel` binary, refresh managed templates in an existing repository safely:

```sh
keel update
keel doctor
```

`keel update` updates only the `AGENTS.md` block, `.keel/rules/index.md`, and `.agents/skills/keel-*` files — it never touches your `.keel/config.yaml`, `.keel/rules/*.md`, or `.keel/tasks/`.

### Prompt for your coding agent (upgrade to Keel 2.x)

```text
Upgrade this repository's Keel setup to the installed Keel 2.x binary (`keel --version`). Do not run `git commit` or `git push`:

1. Check project files (such as `package.json`, CI workflows, or repo-local MCP configs like `.mcp.json` / `.cursor/mcp.json`) for legacy `keel-cli`, `@pdxxxx/keel@1.x`, or standalone `keel-mcp` references; update them to `keel` / `keel mcp` without touching unrelated dependencies.
2. Run `keel update --dry-run`, then `keel update` to refresh managed templates (`AGENTS.md` block, `.keel/rules/index.md`, `.agents/skills/keel-*`) without using `--reset-config`.
3. Inspect `.keel/config.yaml` and `.keel/rules/*.md` for outdated commands or dead globs.
4. Run `keel doctor`, `keel rules check --strict`, and `keel verify`.
5. Report changed files, verification results, and any manual follow-ups.
```

## Day-to-day usage

- **Small edits**: check `keel rules match --git`, make the edit, run `keel verify --git`. No task folder needed.
- **Multi-step work**: run `keel task create "Refactor session refresh" --slug session-refresh`, keep goals and acceptance criteria in `spec.md`, and `keel task archive <id>` when done.
- **Capturing lessons**: promote checks to `.keel/config.yaml` when automatable; otherwise run `keel rules add <id> --summary "..." --paths "..."`.
