# 安装与起步

## 环境要求

- **Node.js >= 20**：运行 CLI（`keel`）及内置 Node 版 MCP 服务（`keel mcp`）。
- **Rust 1.85+**（可选）：仅在需要自行从源码编译独立 `keel-mcp` 二进制时需要；预编译产物可直接从 [GitHub Releases](https://github.com/pdxxxx/keel/releases) 下载。

## 安装

```sh
npm install -g @pdxxxx/keel@latest
```

验证安装：

```sh
keel --version
```

## 初始化仓库

在项目根目录执行：

```sh
keel init
```

这会写入极简、Git 友好的纯文本目录：

```text
.keel/
  config.yaml        # 自动探测的校验门禁（typecheck / lint / test）与记忆源
  rules/
    index.md         # 规则写法说明
  tasks/             # 可选的单文档任务目录
AGENTS.md            # 短托管块（<!-- KEEL:START --> ... <!-- KEEL:END -->）
.agents/skills/      # keel-plan, keel-verify, keel-distill, keel-mem, keel-migrate
```

如果仓库已有 `AGENTS.md`，Keel 只在末尾追加托管块，不会覆盖你原有的内容。

## 日常怎么用

### 小改动

直接改代码。改前跑 `keel rules match --git`（或让 AI 通过 MCP 调 `keel_rules_match`）看有没有避坑规则，改完跑 `keel verify --git` 通过即可，无需建任务。

### 跨会话 / 多步骤复杂任务

需要把目标、Non-Goals 和验收标准落盘以防上下文压缩丢失时，创建一个轻量任务：

```sh
keel task create "Refactor session refresh" --slug session-refresh
```

会在 `.keel/tasks/YYYY-MM-DD-session-refresh/` 生成 `spec.md` 与 `meta.json`。完成后归档：

```sh
keel task archive 2026-11-24-session-refresh
```

### 沉淀踩坑

修完隐蔽 Bug 或确定架构权衡后：

1. 能写成测试、lint 或脚本检查的，直接加进 `.keel/config.yaml` 的 `verify.gates`。
2. 写不成自动检查的隐性约束，补一条带 `paths` 的短规则：

```sh
keel rules add no-float-money \
  --summary "Store monetary amounts as integer minor units" \
  --paths "src/payments/**"
```

尽量保持规则简短并精确限定 `paths`，让 AI 只在碰相关文件时才看到它。
