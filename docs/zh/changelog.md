# 更新日志

## 2.0.0 — 2026-11-24

纯 Rust 单二进制正式版（`keel`）。

- **纯 Rust 单二进制收敛**：将全部 CLI 命令与 13 工具 stdio MCP 服务合并进单个 ~3.6 MiB 可执行文件；彻底移除重复 TypeScript 核心、独立 `keel-mcp` 子 crate 与双端对照脚本。
- **零依赖 npm 分发**：`@pdxxxx/keel` 精简为 ~10 KB 零依赖启动壳，安装时从 GitHub Releases 下载并校验 5 平台预编译二进制（`linux-x86_64`、`linux-aarch64`、`macos-arm64`、`macos-x86_64`、`windows-x86_64.exe`），发版流水线先完成 Release 上传再通过 OIDC Trusted Publishing 发布 npm。
- **安全升级与自检**：新增 `keel update`（只刷新托管模板与 Skills，不碰 `config.yaml`、规则和任务）、`keel doctor`（严格校验配置字段/类型、标记、git 与死路径）、全局 `-C/--root` 与简洁单行 `stderr` 报错。
- **多仓库 MCP 与 13 工具闭环**：全部 13 个工具支持最高优先级按次 `root`、`initialize` `rootUri`/`workspaceFolders`、无家目录防误写保护与 `--lock-root`，新增 `keel_rules_add`、`keel_task_update`、`keel_hotspots` 与 `keel_mem_show`。
- **门禁防挂死保护**：`verify.gates` 新增 `timeoutMs`（默认 120s，超时终止进程树）与 `maxOutputChars`（默认 16,000，保留末尾日志）。
- **记忆、Git 与迁移增强**：新增 `--scope project | all` 过滤与当前项目加权、内置 `antigravity` / `gemini` 只读适配器、嵌套 monorepo Git 路径对齐，以及多平台 Trellis 残留清理（`.agents/skills/trellis-*`、`.codex/`、`.cursor/hooks*`、`.gemini/settings.json`、`CLAUDE.md`）。

## 1.1.0 — 2026-11-24

- 新增按路径增量门禁（`verify.gates[].paths` + `keel verify --git / --staged / --base`）、`keel rules check --stale / --strict`、`keel hotspots` 与 `keel migrate scan / plan / cleanup / rollback`。

## 1.0.0 — 2026-11-24

- 首个正式版本（`init`、`rules`、`task`、`verify`、`mem`、`ablate` / `restore`、`mcp`）。
