---
id: ablate-atomic-backup
summary: Verify backup SHA-256 before mutating repo and refuse restore on conflicts
severity: invariant
paths:
  - src/core/ablate.ts
---

# Transaction-Safe Ablation & Restore

## Why

`keel ablate` deletes or strips tracked engineering files in the user's working tree. Every backup byte must be written and SHA-256 verified outside the repo before any workspace mutation occurs, and `keel restore` must check every path for post-ablation conflicts before writing a single byte (unless `--force` is explicitly requested).
