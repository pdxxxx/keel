# 安装与起步

## 安装

通过 npm 一行安装（自动下载当前平台预编译 Rust 二进制并强制校验 SHA-256）：

```sh
npm install -g @pdxxxx/keel@latest
```

或使用 Rust 1.85+ 从源码直接安装：

```sh
cargo install --git https://github.com/pdxxxx/keel --locked
```

Linux（`x86_64`、`aarch64`）、macOS（`arm64`、`x86_64`）与 Windows（`x86_64`）预编译单文件也可从 [GitHub Releases](https://github.com/pdxxxx/keel/releases) 直接下载。

验证安装：

```sh
keel --version
```

## 初始化仓库

在项目根目录执行：

```sh
keel init
```

这会创建纯文本、Git 友好的极简目录：

```text
.keel/
  config.yaml        # 自动探测的校验门禁（typecheck / lint / test）与记忆源
  rules/
    index.md         # 规则格式说明
  tasks/             # 可选的单文档任务目录
AGENTS.md            # 托管块（<!-- KEEL:START --> ... <!-- KEEL:END -->）
.agents/skills/      # keel-plan, keel-verify, keel-distill, keel-mem, keel-migrate
```

`keel init` 完整保留 `AGENTS.md` 托管块之外的用户正文，并且除非显式传入 `--reset-config`，绝不会覆盖已有的 `.keel/config.yaml`。

## 升级已有 Keel 项目

升级本机 `keel` 二进制后，在老项目根目录执行：

```sh
keel update
keel doctor
```

`keel update` 只安全刷新 `AGENTS.md` 托管块、`.keel/rules/index.md` 和 `.agents/skills/keel-*`，绝不修改你的 `.keel/config.yaml`、规则或任务。

## 日常怎么用

- **小改动**：改前跑 `keel rules match --git`，改完跑 `keel verify --git`，不需要建任务目录。
- **跨会话复杂任务**：`keel task create "Refactor session refresh" --slug session-refresh`，在单份 `spec.md` 里维护目标和验收标准，完成后 `keel task archive <id>`。
- **沉淀踩坑**：能跑命令的进 `.keel/config.yaml` 的 `verify.gates`，代码卡不住的用 `keel rules add <id> --summary "..." --paths "..."` 记一条短规则。
