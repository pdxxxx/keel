# Getting Started

## Install

Install via npm (downloads and SHA-256 verifies the prebuilt native Rust binary for your platform):

```sh
npm install -g @pdxxxx/keel@latest
```

Or build from source with Rust 1.85+:

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

## Upgrading existing projects

After upgrading the `keel` binary, refresh managed templates in an existing repository safely:

```sh
keel update
keel doctor
```

`keel update` updates only the `AGENTS.md` block, `.keel/rules/index.md`, and `.agents/skills/keel-*` files — it never touches your `.keel/config.yaml`, `.keel/rules/*.md`, or `.keel/tasks/`.

## Day-to-day usage

- **Small edits**: check `keel rules match --git`, make the edit, run `keel verify --git`. No task folder needed.
- **Multi-step work**: run `keel task create "Refactor session refresh" --slug session-refresh`, keep goals and acceptance criteria in `spec.md`, and `keel task archive <id>` when done.
- **Capturing lessons**: promote checks to `.keel/config.yaml` when automatable; otherwise run `keel rules add <id> --summary "..." --paths "..."`.
