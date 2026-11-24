---
name: keel-plan
description: Align requirements and persist a concise living task spec in .keel/tasks/ when work is multi-step, ambiguous, or spans sessions.
---

# Keel Plan

Use this skill when a request is ambiguous, multi-step, or likely to span sessions. Skip it for trivial edits.

1. Explore the relevant code first instead of asking questions the codebase already answers.
2. Check existing invariants with `keel rules match <target-paths>` and check prior decisions with `keel mem search "<topic>"`.
3. Create a task when persistence helps: `keel task create "<title>"`.
4. Maintain a single concise `spec.md` covering **Goal**, **Non-Goals**, **Acceptance Criteria**, and key **Decisions**. Put deeper technical notes in `research/` if needed.
5. Execution strategy (inline vs. native sub-agents/swarms) is up to the host harness once requirements are clear.
