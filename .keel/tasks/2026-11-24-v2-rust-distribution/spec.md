# Keel v2 single Rust binary and verified npm distribution

## Goal

- Deliver one native Keel 2.0.0 executable for CLI and MCP, plus a dependency-free npm launcher.

## Non-Goals

- No legacy TypeScript implementation, real release, commits, or edits to user-maintained documentation.

## Acceptance Criteria

- Pass locked Rust tests, Clippy, release build, npm installer tests and dry-run pack.
- Exercise all 13 tools over stdio, project setup/health, scoped Git and memory, transactional cleanup and restore through the CLI.
- Run the release binary's doctor, strict rules check and verify on this repository.

## Decisions & Notes

- Use the current working tree as the migration source. Publish binaries and their SHA-256 manifest before enabling the npm release.
