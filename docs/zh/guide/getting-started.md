# 安装与起步

## 安装

通过 npm 一行安装（自动下载当前平台预编译 Rust 二进制并强制校验 SHA-256）：

```sh
npm install -g @pdxxxx/keel@latest
```

或使用 Rust 1.89+ 从源码直接安装：

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

### 交给 AI 一键初始化（新项目）

```text
请使用本机已安装的 `keel`（先确认 `keel --version` 为 2.x）为当前项目初始化 Keel。全程自主完成，不要执行 `git commit` 或 `git push`：

1. 执行 `keel init`。
2. 核查 `.keel/config.yaml`：对照项目构建脚本与 CI 配置补全真实有效的 `verify.gates`（Monorepo 为模块门禁加上 `paths`）。如果是刚建好的空白仓库，保持 `verify.gates: []`，不要编造假命令。如有长期文档目录（如 `docs/adr`），加入 `memory.sources`。
3. 规则宁缺毋滥：不要写通用风格规范或目录说明；能进门禁的都进 `verify.gates`；只用 `keel rules add --paths` 记录读附近代码猜不出来的暗坑、明确禁令或外部怪癖（每条 10–20 行）。新项目保持 0 条自定义规则完全合理。
4. 执行 `keel doctor`、`keel rules check --strict`，若已配置门禁则执行 `keel verify`。
5. 简要汇报初始化的文件、门禁列表、新增规则及验收结果。
```

## 升级已有 Keel 项目

升级本机 `keel` 二进制后，在老项目根目录执行：

```sh
keel update
keel doctor
```

`keel update` 只安全刷新 `AGENTS.md` 托管块、`.keel/rules/index.md` 和 `.agents/skills/keel-*`，绝不修改你的 `.keel/config.yaml`、规则或任务。

### 交给 AI 一键升级（已有 Keel 项目 → 2.x）

```text
请将当前项目的 Keel 配置与托管文件升级到本机已安装的 Keel 2.x（先确认 `keel --version`）。不要执行 `git commit` 或 `git push`：

1. 检查项目内是否有旧版 `keel-cli`、`@pdxxxx/keel@1.x` 或独立 `keel-mcp` 引用（包括 `package.json`、CI 脚本与项目级 `.mcp.json` / `.cursor/mcp.json` 等）；如有，按单二进制 `keel` / `keel mcp` 修正，不动无关配置。
2. 先执行 `keel update --dry-run`，再执行 `keel update` 刷新托管文件（严禁使用 `--reset-config` 覆盖现有 `.keel/config.yaml`）。
3. 检查 `.keel/config.yaml` 与 `.keel/rules/*.md` 是否有过期命令或失效 `paths`。
4. 执行 `keel doctor`、`keel rules check --strict` 和 `keel verify`。
5. 简要汇报变更文件、验收结果以及是否还有需要人工确认的残留项。
```

## 日常怎么用

- **小改动**：改前跑 `keel rules match --git`，改完跑 `keel verify --git`，不需要建任务目录。
- **跨会话复杂任务**：`keel task create "Refactor session refresh" --slug session-refresh`，在单份 `spec.md` 里维护目标和验收标准，完成后 `keel task archive <id>`。
- **沉淀踩坑**：能跑命令的进 `.keel/config.yaml` 的 `verify.gates`，代码卡不住的用 `keel rules add <id> --summary "..." --paths "..."` 记一条短规则。
