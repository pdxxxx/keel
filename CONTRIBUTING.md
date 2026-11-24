# Contributing

## Requirements

- Rust 1.85+ with `clippy`
- Node.js >= 20 (only for running the thin npm wrapper tests in `npm/test.mjs` or previewing `docs/`)

## Local checks

```sh
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
node --test npm/test.mjs
```

Or run Keel's own verification gates:

```sh
cargo run --quiet -- doctor
cargo run --quiet -- verify
```

## Guidelines

- Keep runtime dependencies minimal and cross-platform (Windows, macOS, Linux).
- Do not add sub-agent orchestration rules or vendor hook logic.
- Keep `stdout` strictly clean for `--json` and `keel mcp` stdio frames (diagnostics go to `stderr`).
- Check `.keel/rules/` with `cargo run --quiet -- rules match --git` before opening a PR.
