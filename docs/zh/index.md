---
layout: home

hero:
  name: keel
  text: 面向 AI 编程的仓库规则、记忆与校验门禁。
  tagline: 单个 3.6 MiB 纯 Rust 可执行文件搞定全部 CLI 与 13 个 stdio MCP 工具。纯文本存入 .keel/、7 源流式会话检索、确定性门禁，零调度锁定。
  image:
    src: /keel.png
    alt: keel
  actions:
    - theme: brand
      text: 快速开始
      link: /zh/guide/getting-started
    - theme: alt
      text: MCP 服务
      link: /zh/guide/mcp
    - theme: alt
      text: GitHub
      link: https://github.com/pdxxxx/keel

features:
  - title: 纯 Rust 单二进制
    details: 一个可执行文件同时承载全部 CLI 与 13 个 stdio MCP 工具，不依赖 Node 或 Python 运行时；支持 npm、Cargo 或 GitHub Releases 安装。
  - title: 按路径规则与安全升级
    details: 规则按路径存在 .keel/rules/，只拉取命中当前 git diff 的项；keel doctor 严格排查配置错误，keel update 升级模板不碰用户配置。
  - title: 原生支持多仓库与 MCP 聚合器
    details: 单个常驻 keel mcp 进程通过按次传入 root 安全服务多个仓库，自带无家目录防误写保护与可选 --lock-root 沙箱锁。
  - title: 7 源流式跨工具记忆
    details: 原地只读扫描 .keel/、Claude Code、Codex、OpenCode、Antigravity、Gemini 与自定义目录，支持 --scope project 过滤与当前项目加权。
---

<div class="keel-home-section">

<div class="keel-stat-grid">
  <div class="keel-stat-card">
    <div class="keel-stat-value">3.6 MiB</div>
    <div class="keel-stat-label">纯 Rust 单文件二进制（CLI + 13 个 MCP 工具）</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">3.2 ms</div>
    <div class="keel-stat-label">stdio MCP 握手中位耗时</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">7 类来源</div>
    <div class="keel-stat-label">零数据库流式只读会话检索</div>
  </div>
  <div class="keel-stat-card">
    <div class="keel-stat-value">5 平台</div>
    <div class="keel-stat-label">Linux / macOS / Windows 预编译发布</div>
  </div>
</div>

### 五步日常闭环

```sh
npm install -g @pdxxxx/keel@latest

keel init                                    # 创建 .keel/、AGENTS.md 块与 5 个标准 skills
keel rules add auth-rotation \               # 记录代码里看不出来的暗坑
  --summary "Rotate refresh tokens atomically" \
  --paths "src/auth/**"
keel rules match --git                       # 拉取命中当前变更的规则
keel mem search "refresh token" --scope project
keel verify --git                            # 运行命中当前变更的检查门禁
```

</div>
