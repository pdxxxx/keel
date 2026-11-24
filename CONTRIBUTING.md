# Contributing

## Setup

Requires Node.js >= 20 and pnpm. If you are touching `crates/keel-mcp`, you also need Rust 1.85+ with `clippy`.

```sh
pnpm install
pnpm build
pnpm test
```

## Running checks

Run the repository's own verification gates:

```sh
node bin/keel.js verify
```

If you changed `src/core/mcp.ts` or anything under `crates/keel-mcp/`, also run the Rust tests and parity script:

```sh
cargo test --locked --manifest-path crates/keel-mcp/Cargo.toml
cargo clippy --all-targets --manifest-path crates/keel-mcp/Cargo.toml -- -D warnings
cargo build --release --locked --manifest-path crates/keel-mcp/Cargo.toml
node crates/keel-mcp/tests/parity.mjs
```

## Guidelines

- Keep runtime dependencies minimal and cross-platform (Windows, macOS, Linux).
- Do not add sub-agent orchestration rules or vendor-specific hook logic.
- Any change to `.keel/` file formats or MCP tool behavior must stay consistent between TypeScript (`src/core/`) and Rust (`crates/keel-mcp/`) and pass `parity.mjs`.
- Check `.keel/rules/` with `node bin/keel.js rules match --git` before opening a PR.
