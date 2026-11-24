---
layout: home

hero:
  name: keel
  text: 面向 AI 编程的仓库规则、记忆与校验门禁。
  tagline: 纯文本存入 .keel/、跨工具检索本地会话、按路径增量校验，以及约 2 MiB 的原生 Rust MCP 服务。不接管调度，不绑死工具。
  image:
    src: /keel.png
    alt: keel
  actions:
    - theme: brand
      text: 快速开始
      link: /zh/guide/getting-started
    - theme: alt
      text: 原生 Rust MCP
      link: /zh/guide/mcp
    - theme: alt
      text: GitHub
      link: https://github.com/pdxxxx/keel

features:
  - title: 按路径按需拉取规则
    details: 把暗坑与不变量以 Markdown 存在 .keel/rules/，只拉取命中当前 git diff 的规则；用 keel rules check --stale 清理失效 glob。
  - title: 确定性增量验证门禁
    details: 用 .keel/config.yaml 运行真实的 typecheck、lint、test 命令；为门禁加上 paths 后，keel verify --git 在大仓库里也能秒跑。
  - title: 跨工具本地会话记忆
    details: 直接检索 Claude Code、Codex、OpenCode、自定义目录 JSONL 会话以及 .keel/ 文档，不再手动写流水账日记。
  - title: 零调度锁定
    details: 不强制子 Agent 拓扑，不装宿主私有 PreToolUse Hook，零 Python 脚本。把执行与并行调度权留给现代模型和底座。
---

<div class="keel-home-section">

<div class="keel-stat-grid">
  <div class="keel-stat-card">
    <div class="keel-stat-value">~2 MiB</div>
    <div class="keel-stat-label">单文件 Rust MCP 二进制（无需 Node 运行时）</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">3.2 ms</div>
    <div class="keel-stat-label">Windows 实测 stdio MCP 握手中位耗时</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">43 ms</div>
    <div class="keel-stat-label">检索 98,304 条 JSONL 记录并返回前 15 条</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">61 项</div>
    <div class="keel-stat-label">CI 双平台 Node 与 Rust stdio 响应对照</div>
  </div>
</div>

### 五步日常闭环

```sh
npm install -g @pdxxxx/keel@latest

keel init                                    # 创建 .keel/、AGENTS.md 块与标准 skills
keel rules add auth-rotation \               # 记录代码里看不出来的暗坑
  --summary "Rotate refresh tokens atomically" \
  --paths "src/auth/**"
keel rules match --git                       # 拉取命中当前变更的规则
keel mem search "refresh token"              # 跨工具检索历史会话与 .keel/ 文档
keel verify --git                            # 运行命中当前变更的检查门禁
```

</div>
