---
layout: home

hero:
  name: keel
  text: Engineering rules, memory, and checks for coding agents.
  tagline: One 3.6 MiB Rust binary for CLI and stdio MCP. Plain files under .keel/, streaming 7-source session search, deterministic gates, and zero sub-agent lock-in.
  image:
    src: /keel.png
    alt: keel
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: MCP Server
      link: /guide/mcp
    - theme: alt
      text: GitHub
      link: https://github.com/pdxxxx/keel

features:
  - title: Single Rust Binary
    details: One executable handles both the CLI and all 13 stdio MCP tools. No Node or Python runtime required; install via npm, Cargo, or GitHub Releases.
  - title: Scoped Rules & Safe Updates
    details: Keep path-scoped rules under .keel/rules/, pull only what matches your git diff, validate with keel doctor, and refresh templates safely with keel update.
  - title: Multi-Repo MCP Built-In
    details: One shared keel mcp process serves multiple repos safely behind MCP aggregators via per-call root, homeless-cwd write guards, and optional --lock-root.
  - title: Streaming Cross-Tool Memory
    details: Search .keel/, Claude Code, Codex, OpenCode, Antigravity, Gemini, and custom logs with --scope project filtering and current-repo ranking boost.
---

<div class="keel-home-section">

<div class="keel-stat-grid">
  <div class="keel-stat-card">
    <div class="keel-stat-value">3.6 MiB</div>
    <div class="keel-stat-label">Single pure-Rust binary (CLI + 13 MCP tools)</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">3.2 ms</div>
    <div class="keel-stat-label">Median stdio MCP handshake</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">7 sources</div>
    <div class="keel-stat-label">Streaming read-only memory search</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">5 targets</div>
    <div class="keel-stat-label">Linux, macOS & Windows prebuilt releases</div>
  </div>
</div>

### Five-command loop

```sh
npm install -g @pdxxxx/keel@latest

keel init                                    # create .keel/, AGENTS.md block, 5 skills
keel rules add auth-rotation \               # record an undiscoverable trap
  --summary "Rotate refresh tokens atomically" \
  --paths "src/auth/**"
keel rules match --git                       # pull rules matching current diff
keel mem search "refresh token" --scope project
keel verify --git                            # run gates matching current diff
```

</div>
