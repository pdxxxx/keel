---
name: keel-verify
description: Verify changes against deterministic project gates, matched Keel invariants, and task acceptance criteria before finishing.
---

# Keel Verify

Run this before declaring work complete:

1. Execute `keel verify` to run all configured deterministic gates (typecheck, lint, tests, invariant checks).
2. Run `keel rules match --git` and confirm the diff respects every matched invariant.
3. If working on an active task under `.keel/tasks/`, verify every acceptance criterion in `spec.md`.
4. For high-risk or cross-module changes, prefer a fresh-context review of the diff to catch blind spots.
