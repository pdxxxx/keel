# Engineering Rules & Invariants

Keep this directory high-signal.

**What belongs here**
- Non-obvious architectural invariants (`severity: invariant`)
- Historical footguns and negative constraints ("Don't do X because Y")
- Architecture Decision Records (`severity: adr`)

**What does NOT belong here**
- Conventions obvious from reading two nearby files
- Rules that can be enforced automatically by a linter, type-checker, or test (add those to `.keel/config.yaml` under `verify.gates` instead)

## Rule File Format

```markdown
---
id: short-kebab-id
summary: One-line summary shown by `keel rules match`
severity: invariant # invariant | guideline | adr
paths:
  - src/module/**
---

Explain the invariant and *why* it exists.
```
