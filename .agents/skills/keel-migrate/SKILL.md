---
name: keel-migrate
description: Migrate a repository from Trellis, .cursorrules, CLAUDE.md, or other AI workflow setups to Keel by distilling high-signal rules, configuring gates, and cleaning up conflicting hooks.
---

# Keel Migrate

Use this skill once when moving an existing repository from another AI coding setup (Trellis, `.cursorrules`, `CLAUDE.md`, Copilot instructions, Spec-Kit, BMAD) to Keel.

## Steps

1. Run `keel init` if `.keel/` does not exist yet.
2. Run `keel migrate scan --json` and `keel migrate plan --json` to inventory every legacy file, active hook conflict, candidate gate, and archive directory.
3. Configure `.keel/config.yaml`:
   - Add real project checks under `verify.gates` (use `paths` on module-specific gates in monorepos so `keel verify --git` stays fast).
   - Add existing ADR or archive directories (such as `docs/adr` or `.trellis/tasks/archive`) to `memory.sources` instead of copying them.
4. Distill legacy specs 10:1 into `.keel/rules/`:
   - **Drop** generic coding style guides, directory overviews, boilerplate checklists, and old sub-agent/workflow instructions.
   - **Promote to gates** anything that can be checked by a command or test.
   - **Keep as scoped rules** (`keel rules add <id> --summary "..." --paths "..."`) only things an engineer or model cannot infer from reading nearby code: negative constraints ("never do X because Y"), historical bug traps, external system quirks, and tie-breakers where legacy and new patterns coexist. Keep each rule concise (~10-20 lines).
5. If there is an active in-progress task in the old tool, consolidate its goal, non-goals, and acceptance criteria into one `spec.md` with `keel task create`. Leave archived tasks untouched.
6. Run `keel rules check --stale` and `keel verify` to confirm the new setup passes cleanly.
7. Preview conflict removal with `keel migrate cleanup --dry-run`. Ask the user to confirm before running `keel migrate cleanup` (which backs up removed files outside the repo and can be undone with `keel migrate rollback`).
