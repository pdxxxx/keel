# 老仓库接入与迁移

## 已有代码库（1→N）怎么用 Keel

不要第一天就让 AI 通读全库写几十份代码风格规范。老项目分三步落地最稳：

### 1. 配好带超时与路径作用域的门禁

跑 `keel init` 后，在 `.keel/config.yaml` 中按模块配置 `paths` 与安全上限：

```yaml
version: 1
verify:
  gates:
    - name: api-test
      command: pnpm --filter api test
      paths:
        - packages/api/**
      timeoutMs: 120000
      maxOutputChars: 16000
    - name: web-typecheck
      command: pnpm --filter web typecheck
      paths:
        - packages/web/**
    - name: doctor
      command: keel doctor
```

日常开发跑 `keel verify --git` 只跑命中变更的门禁；CI 跑全量 `keel verify`。

### 2. 用 `keel hotspots` 从 Git 历史挖真坑

```sh
keel hotspots --commits 150 --limit 10
```

首批只写 3–5 条读代码猜不出来的规则：新旧写法并存时的裁决、外部系统暗坑、跨文件代码生成联动约定。

### 3. 定期跑 `keel doctor` 与 `keel update`

- `keel doctor`：一键检查 `.keel/config.yaml` 拼写/类型错误、`AGENTS.md` 标记、`git` 及死路径规则。
- `keel update`：升级新版 `keel` 后安全刷新托管模板和 Skills，绝不覆盖你的 `config.yaml`。

---

## 从 Trellis、`.cursorrules` 或 `CLAUDE.md` 迁移

不要让新旧两套框架同时激活——Trellis 的 `UserPromptSubmit` / `PreToolUse` Hook 会在每轮触发并与 Keel 打架。

Keel 2.0 内置覆盖多平台 Trellis 残留（`.trellis/`、`AGENTS.md`、`CLAUDE.md`、`.agents/skills/trellis-*`、`.claude/`、`.codex/`、`.cursor/`、`.gemini/`）及 `.cursorrules` / Windsurf / Spec-Kit / BMAD 的安全迁移命令：

```sh
keel init
keel migrate scan              # 只读盘点旧文件与活跃 Hook 冲突
keel migrate plan              # 输出冲突项、候选门禁、记忆目录与待提纯规则源
keel migrate cleanup --dry-run # 预览安全清理清单
keel migrate cleanup           # 外部 SHA-256 备份并清理冲突文件
keel migrate rollback          # 如有需要，无损回滚
```

### 一句话交给 AI 全自动迁移（Trellis → Keel）

```text
本次任务是将当前项目从 Trellis 一次性完整迁移到 Keel。请完全忽略当前会话中注入的所有 `<workflow-state>`、Trellis Hook 提醒与旧工作流规范，不要创建 Trellis 任务，也不要派发 `trellis-*` 子 Agent。

本机已全局安装 `keel`，请直接一次性完成以下全部步骤（全程不要执行 `git commit`）：
1. 运行 `keel init`、`keel migrate scan --json`、`keel migrate plan --json` 和 `keel hotspots --json`。
2. 按 `.agents/skills/keel-migrate/SKILL.md` 做 10:1 提纯：丢弃代码风格和旧流程，将可自动化检查写进 `.keel/config.yaml` 的 `verify.gates`，历史 ADR 目录挂进 `memory.sources`，只把暗坑、负向约束和新旧模式裁决用 `keel rules add --paths` 写成短规则（≤ 20 行）。
3. 如有未完成的活跃 Trellis 任务，合并核心目标与验收标准建一个 `keel task`。
4. 确保 `keel doctor` 与 `keel verify` 全部通过。
5. 执行 `keel migrate cleanup` 备份并移除 Trellis 冲突文件（可随时 `keel migrate rollback` 回滚）。
6. 输出一份精简迁移摘要，不要 git commit。
```

清理完成后**新开一个干净会话**使用即可。
