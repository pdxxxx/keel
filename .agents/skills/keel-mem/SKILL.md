---
name: keel-mem
description: Recall past decisions, debugging trails, and rationale across local AI coding sessions and repository specs using keel mem.
---

# Keel Mem

Use this whenever someone asks "why did we do X?", "how did we fix this before?", or before refactoring unfamiliar code.

1. Run `keel mem search "<keywords>"` to search across local AI transcripts and `.keel/` task/rule artifacts.
2. Inspect full context for a relevant session with `keel mem show <session-id>`.
3. Quote concrete rationale rather than guessing; if the decision matters long-term, distill it into `.keel/rules/`.
