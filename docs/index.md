---
layout: home

hero:
  name: keel
  text: Engineering rules, memory, and checks for coding agents.
  tagline: Plain files under .keel/, local session search, deterministic verification gates, and a 2 MiB Rust MCP server. Zero sub-agent lock-in.
  image:
    src: /keel.png
    alt: keel
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: Native Rust MCP
      link: /guide/mcp
    - theme: alt
      text: GitHub
      link: https://github.com/pdxxxx/keel

features:
  - title: Scoped Rules, Pulled on Demand
    details: Keep path-scoped Markdown rules under .keel/rules/ and pull only what matches your git diff. Check dead globs with keel rules check --stale.
  - title: Deterministic Verification Gates
    details: Run real typecheck, lint, and test commands from .keel/config.yaml. Scope gates by path globs so keel verify --git stays fast in monorepos.
  - title: Cross-Tool Session Memory
    details: Search local Claude Code, Codex, OpenCode, and custom JSONL transcripts together with .keel/ docs. No manual session journals required.
  - title: Zero Orchestration Lock-In
    details: No custom sub-agent topologies, no vendor PreToolUse hooks, no Python scripts. Let your coding tool schedule work its own way.
---

<div class="keel-home-section">

<div class="keel-stat-grid">
  <div class="keel-stat-card">
    <div class="keel-stat-value">~2 MiB</div>
    <div class="keel-stat-label">Single-binary Rust MCP server (no Node runtime)</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">3.2 ms</div>
    <div class="keel-stat-label">Median stdio MCP handshake on Windows</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">43 ms</div>
    <div class="keel-stat-label">Search across 98,304 JSONL turns (top 15)</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">61 checks</div>
    <div class="keel-stat-label">Node & Rust stdio parity suite in CI</div>
  </div>
</div>

### Five-command loop

```sh
npm install -g @pdxxxx/keel@latest

keel init                                    # create .keel/, AGENTS.md block, standard skills
keel rules add auth-rotation \               # record an undiscoverable trap
  --summary "Rotate refresh tokens atomically" \
  --paths "src/auth/**"
keel rules match --git                       # pull rules matching current diff
keel mem search "refresh token"              # search local AI transcripts & .keel/ docs
keel verify --git                            # run gates matching current diff
```

</div>
