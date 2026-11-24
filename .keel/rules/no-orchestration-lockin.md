---
id: no-orchestration-lockin
summary: Never enforce sub-agent topologies or vendor hook interception in core
severity: invariant
paths:
  - src/**
---

# Zero Orchestration Lock-In

## Why

Modern AI coding harnesses evolve native sub-agent, swarm, and worktree scheduling rapidly. Hard-coding sub-agent roles or intercepting vendor-specific `PreToolUse` hooks creates control-plane conflict and brittle maintenance debt. Keel stays strictly in the repository contract, memory, and deterministic verification layer.
