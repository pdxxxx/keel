---
name: keel-distill
description: Distill non-obvious engineering lessons, bug root causes, or architectural decisions into executable gates or concise .keel/rules/ entries.
---

# Keel Distill

Use this after solving a tricky bug, finishing an architectural change, or resolving a recurring review comment.

1. Ask first: **Can this be enforced deterministically** (a unit test, invariant test, lint rule, or schema check)? If yes, write the check and ensure it runs under `keel verify`.
2. Otherwise, if the insight cannot be inferred just from reading the code, add or update a concise scoped rule via `keel rules add <id> --summary "..." --paths "..."` and explain **Why**.
3. Keep rules brief and delete obsolete ones — stale rules hurt agent performance.
