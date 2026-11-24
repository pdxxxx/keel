<!-- KEEL:START -->
# Keel Engineering Context

This repository uses **Keel** for scoped engineering invariants, lightweight task contracts, cross-session memory, and deterministic verification.

- **Before editing non-trivial files**: pull matching invariants via `keel rules match <files...>` (or `keel rules match --git`).
- **Need past context or decisions?** Search cross-tool transcripts and repo specs via `keel mem search "<query>"`.
- **For multi-step or cross-session work**: persist scope and acceptance criteria in `.keel/tasks/<id>/spec.md` via `keel task create "<title>"`. Small edits do not need a task.
- **Before finishing**: run `keel verify` to execute deterministic project gates.
- **Execution is up to you**: Keel never prescribes whether to edit inline or dispatch native sub-agents/swarms — optimize for accuracy and speed, and satisfy the contracts.
- **After fixing a non-obvious bug or settling an architectural tradeoff**: distill the invariant into `.keel/rules/` (`keel rules add`) or promote it into an executable verification gate.
<!-- KEEL:END -->
