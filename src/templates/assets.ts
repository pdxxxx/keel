import YAML from "yaml";
import type { VerifyGateConfig } from "../core/types.js";

export const AGENTS_START_MARKER = "<!-- KEEL:START -->";
export const AGENTS_END_MARKER = "<!-- KEEL:END -->";

export function renderAgentsBlock(): string {
  return `${AGENTS_START_MARKER}
# Keel Engineering Context

This repository uses **Keel** for scoped engineering invariants, lightweight task contracts, cross-session memory, and deterministic verification.

- **Before editing non-trivial files**: pull matching invariants via \`keel rules match <files...>\` (or \`keel rules match --git\`).
- **Need past context or decisions?** Search cross-tool transcripts and repo specs via \`keel mem search "<query>"\`.
- **For multi-step or cross-session work**: persist scope and acceptance criteria in \`.keel/tasks/<id>/spec.md\` via \`keel task create "<title>"\`. Small edits do not need a task.
- **Before finishing**: run \`keel verify\` to execute deterministic project gates.
- **Execution is up to you**: Keel never prescribes whether to edit inline or dispatch native sub-agents/swarms — optimize for accuracy and speed, and satisfy the contracts.
- **After fixing a non-obvious bug or settling an architectural tradeoff**: distill the invariant into \`.keel/rules/\` (\`keel rules add\`) or promote it into an executable verification gate.
${AGENTS_END_MARKER}`;
}

export function renderDefaultConfig(gates: VerifyGateConfig[] = []): string {
  if (gates.length === 0) {
    return `# Keel configuration
version: 1

verify:
  gates:
    # Add deterministic project checks here, e.g.:
    # - name: typecheck
    #   command: pnpm typecheck
    # - name: test
    #   command: pnpm test
    []

memory:
  sources: []
`;
  }

  const doc = {
    version: 1,
    verify: {
      gates: gates.map((g) => ({
        name: g.name,
        command: g.command,
        ...(g.cwd ? { cwd: g.cwd } : {}),
        ...(g.paths ? { paths: g.paths } : {}),
      })),
    },
    memory: {
      sources: [],
    },
  };

  return `# Keel configuration\n${YAML.stringify(doc)}`;
}

export function renderRulesIndex(): string {
  return `# Engineering Rules & Invariants

Keep this directory high-signal.

**What belongs here**
- Non-obvious architectural invariants (\`severity: invariant\`)
- Historical footguns and negative constraints ("Don't do X because Y")
- Architecture Decision Records (\`severity: adr\`)

**What does NOT belong here**
- Conventions obvious from reading two nearby files
- Rules that can be enforced automatically by a linter, type-checker, or test (add those to \`.keel/config.yaml\` under \`verify.gates\` instead)

## Rule File Format

\`\`\`markdown
---
id: short-kebab-id
summary: One-line summary shown by \`keel rules match\`
severity: invariant # invariant | guideline | adr
paths:
  - src/module/**
---

Explain the invariant and *why* it exists.
\`\`\`
`;
}

export interface StandardSkillAsset {
  dirName: string;
  content: string;
}

export function getStandardSkills(): StandardSkillAsset[] {
  return [
    {
      dirName: "keel-plan",
      content: `---
name: keel-plan
description: Align requirements and persist a concise living task spec in .keel/tasks/ when work is multi-step, ambiguous, or spans sessions.
---

# Keel Plan

Use this skill when a request is ambiguous, multi-step, or likely to span sessions. Skip it for trivial edits.

1. Explore the relevant code first instead of asking questions the codebase already answers.
2. Check existing invariants with \`keel rules match <target-paths>\` and check prior decisions with \`keel mem search "<topic>"\`.
3. Create a task when persistence helps: \`keel task create "<title>"\`.
4. Maintain a single concise \`spec.md\` covering **Goal**, **Non-Goals**, **Acceptance Criteria**, and key **Decisions**. Put deeper technical notes in \`research/\` if needed.
5. Execution strategy (inline vs. native sub-agents/swarms) is up to the host harness once requirements are clear.
`,
    },
    {
      dirName: "keel-verify",
      content: `---
name: keel-verify
description: Verify changes against deterministic project gates, matched Keel invariants, and task acceptance criteria before finishing.
---

# Keel Verify

Run this before declaring work complete:

1. Execute \`keel verify\` to run all configured deterministic gates (typecheck, lint, tests, invariant checks).
2. Run \`keel rules match --git\` and confirm the diff respects every matched invariant.
3. If working on an active task under \`.keel/tasks/\`, verify every acceptance criterion in \`spec.md\`.
4. For high-risk or cross-module changes, prefer a fresh-context review of the diff to catch blind spots.
`,
    },
    {
      dirName: "keel-distill",
      content: `---
name: keel-distill
description: Distill non-obvious engineering lessons, bug root causes, or architectural decisions into executable gates or concise .keel/rules/ entries.
---

# Keel Distill

Use this after solving a tricky bug, finishing an architectural change, or resolving a recurring review comment.

1. Ask first: **Can this be enforced deterministically** (a unit test, invariant test, lint rule, or schema check)? If yes, write the check and ensure it runs under \`keel verify\`.
2. Otherwise, if the insight cannot be inferred just from reading the code, add or update a concise scoped rule via \`keel rules add <id> --summary "..." --paths "..."\` and explain **Why**.
3. Keep rules brief and delete obsolete ones — stale rules hurt agent performance.
`,
    },
    {
      dirName: "keel-mem",
      content: `---
name: keel-mem
description: Recall past decisions, debugging trails, and rationale across local AI coding sessions and repository specs using keel mem.
---

# Keel Mem

Use this whenever someone asks "why did we do X?", "how did we fix this before?", or before refactoring unfamiliar code.

1. Run \`keel mem search "<keywords>"\` to search across local AI transcripts and \`.keel/\` task/rule artifacts.
2. Inspect full context for a relevant session with \`keel mem show <session-id>\`.
3. Quote concrete rationale rather than guessing; if the decision matters long-term, distill it into \`.keel/rules/\`.
`,
    },
    {
      dirName: "keel-migrate",
      content: `---
name: keel-migrate
description: Migrate a repository from Trellis, .cursorrules, CLAUDE.md, or other AI workflow setups to Keel by distilling high-signal rules, configuring gates, and cleaning up conflicting hooks.
---

# Keel Migrate

Use this skill once when moving an existing repository from another AI coding setup (Trellis, \`.cursorrules\`, \`CLAUDE.md\`, Copilot instructions, Spec-Kit, BMAD) to Keel.

## Steps

1. Run \`keel init\` if \`.keel/\` does not exist yet.
2. Run \`keel migrate scan --json\` and \`keel migrate plan --json\` to inventory every legacy file, active hook conflict, candidate gate, and archive directory.
3. Configure \`.keel/config.yaml\`:
   - Add real project checks under \`verify.gates\` (use \`paths\` on module-specific gates in monorepos so \`keel verify --git\` stays fast).
   - Add existing ADR or archive directories (such as \`docs/adr\` or \`.trellis/tasks/archive\`) to \`memory.sources\` instead of copying them.
4. Distill legacy specs 10:1 into \`.keel/rules/\`:
   - **Drop** generic coding style guides, directory overviews, boilerplate checklists, and old sub-agent/workflow instructions.
   - **Promote to gates** anything that can be checked by a command or test.
   - **Keep as scoped rules** (\`keel rules add <id> --summary "..." --paths "..."\`) only things an engineer or model cannot infer from reading nearby code: negative constraints ("never do X because Y"), historical bug traps, external system quirks, and tie-breakers where legacy and new patterns coexist. Keep each rule concise (~10-20 lines).
5. If there is an active in-progress task in the old tool, consolidate its goal, non-goals, and acceptance criteria into one \`spec.md\` with \`keel task create\`. Leave archived tasks untouched.
6. Run \`keel rules check --stale\` and \`keel verify\` to confirm the new setup passes cleanly.
7. Preview conflict removal with \`keel migrate cleanup --dry-run\`. Ask the user to confirm before running \`keel migrate cleanup\` (which backs up removed files outside the repo and can be undone with \`keel migrate rollback\`).
`,
    },
  ];
}
