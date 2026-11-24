# 老仓库接入与迁移

## 已有代码库（1→N）怎么用 Keel

不要第一天就让 AI 通读全库写几十份“代码风格指南”——强模型读两三个相邻文件就能模仿本地风格，冗长风格文档很快就会过时。

老项目分三步落地最稳：

### 1. 先配按路径触发的增量门禁

跑 `keel init` 后，在 `.keel/config.yaml` 里按子包/模块配置 `paths`：

```yaml
version: 1
verify:
  gates:
    - name: api-test
      command: pnpm --filter api test
      paths:
        - packages/api/**
    - name: web-typecheck
      command: pnpm --filter web typecheck
      paths:
        - packages/web/**
    - name: rules-check
      command: keel rules check --stale
```

日常开发跑 `keel verify --git`，只跑命中变更的门禁；CI 或发版前跑完整 `keel verify`。

### 2. 用 `keel hotspots` 从 Git 历史挖真坑

找出最容易反复出 Bug 的文件：

```sh
keel hotspots --commits 150 --limit 10
```

首批只写 3–5 条“读代码猜不出来”的规则：
- **新旧模式并存时的裁决**（“新状态统一用 Zustand，不要模仿 `src/legacy/**` 里的 Redux 写法”）。
- **外部系统或线上暗坑**（“不要改这个字段名，下游对账脚本硬编码依赖它”）。
- **跨文件生成物联动**（“改了 `proto/**` 必须同步执行 `pnpm gen:sdk`”）。

### 3. 定期跑规则体检

定期运行 `keel rules check --stale`（或在 CI 中跑 `--strict`），清理重构后失效的死路径 glob、超长规则和过多全局规则。

---

## 从 Trellis、`.cursorrules` 或 `CLAUDE.md` 迁移

切忌让两套框架同时激活——Trellis 的 `UserPromptSubmit` / `PreToolUse` Hook 会在每轮继续触发，和 Keel 互相打架。

Keel 1.1 内置确定性迁移命令和 `keel-migrate` skill：

```sh
keel init
keel migrate scan              # 只读盘点旧文件与活跃 Hook 冲突
keel migrate plan              # 输出冲突项、候选门禁、记忆目录与待提纯规则源
keel migrate cleanup --dry-run # 预览安全清理清单
keel migrate cleanup           # 外部 SHA-256 备份并清理冲突文件
keel migrate rollback          # 如有需要，无损回滚
```

### 一句话交给 AI 全自动迁移（Trellis → Keel）

在目标项目里直接发给 AI（自带忽略旧 Hook 覆盖指令，全程不 `git commit`）：

```text
本次任务是将当前项目从 Trellis 一次性完整迁移到 Keel。请完全忽略当前会话中注入的所有 `<workflow-state>`、Trellis Hook 提醒与旧工作流规范，不要创建 Trellis 任务，也不要派发 `trellis-*` 子 Agent。

本机已全局安装 `keel` CLI 与 `keel-mcp`，请直接一次性完成以下全部步骤（全程不要执行 `git commit`）：
1. 运行 `keel init`、`keel migrate scan --json`、`keel migrate plan --json` 和 `keel hotspots --json`。
2. 按 `.agents/skills/keel-migrate/SKILL.md` 做 10:1 提纯：丢弃代码风格和旧流程，将可自动化检查写进 `.keel/config.yaml` 的 `verify.gates`，历史 ADR 目录挂进 `memory.sources`，只把暗坑、负向约束和新旧模式裁决用 `keel rules add --paths` 写成短规则（≤ 20 行）。
3. 如有未完成的活跃 Trellis 任务，合并核心目标与验收标准建一个 `keel task`。
4. 确保 `keel rules check --strict` 与 `keel verify` 全部通过。
5. 执行 `keel migrate cleanup` 备份并移除 Trellis 冲突文件，并核查 `.claude/settings.json`、`.codex/`、`.cursor/` 中没有残留的 Trellis Hook。
6. 输出一份精简迁移摘要，不要 git commit。
```

迁移完清理后，**新开一个干净会话**开始使用即可。
