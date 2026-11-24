import fs from "node:fs";
import path from "node:path";
import { Command } from "commander";
import { ablateRepo, getAblateStatus, restoreRepo } from "./core/ablate.js";
import { getGitChangedFiles } from "./core/git.js";
import { initRepo } from "./core/init.js";
import { startMcpStdioServer } from "./core/mcp.js";
import {
  getMemorySession,
  listMemorySessions,
  listMemorySources,
  searchMemory,
} from "./core/mem.js";
import { resolveRepoRoot } from "./core/paths.js";
import { addRule, listRules, matchRules, validateRules } from "./core/rules.js";
import {
  archiveTask,
  createTask,
  getTask,
  listTasks,
  setTaskStatus,
} from "./core/tasks.js";
import type { RuleSeverity } from "./core/types.js";
import { runVerify } from "./core/verify.js";
import { KEEL_VERSION } from "./version.js";

function printJson(value: unknown): void {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
}

function parsePositiveInt(raw: string | undefined, fallback: number): number {
  if (!raw) return fallback;
  const parsed = Number.parseInt(raw, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

export function createProgram(): Command {
  const program = new Command();

  program
    .name("keel")
    .description(
      "Harness-agnostic engineering context, memory, and verification layer for AI coding.",
    )
    .version(KEEL_VERSION);

  // keel init
  program
    .command("init")
    .description(
      "Initialize .keel/, managed AGENTS.md block, and standard .agents/skills/",
    )
    .option("-f, --force", "Overwrite existing Keel template files")
    .option("--no-skills", "Skip installing standard .agents/skills/")
    .option("--no-detect", "Skip auto-detecting verification gates")
    .option("--json", "Output JSON")
    .action(
      (opts: {
        force?: boolean;
        skills?: boolean;
        detect?: boolean;
        json?: boolean;
      }) => {
        const root = process.cwd();
        const result = initRepo(root, {
          force: opts.force,
          skipSkills: opts.skills === false,
          detectGates: opts.detect !== false,
        });

        if (opts.json) {
          printJson(result);
          return;
        }

        if (result.created.length > 0) {
          console.log("Created:");
          for (const f of result.created) console.log(`  + ${f}`);
        }
        if (result.updated.length > 0) {
          console.log("Updated:");
          for (const f of result.updated) console.log(`  ~ ${f}`);
        }
        if (result.skipped.length > 0) {
          console.log("Skipped (already exists):");
          for (const f of result.skipped) console.log(`  - ${f}`);
        }
        if (result.detectedGates.length > 0) {
          console.log("Detected verification gates:");
          for (const g of result.detectedGates) {
            console.log(`  * ${g.name}: ${g.command}`);
          }
        }
      },
    );

  // keel rules
  const rulesCmd = program
    .command("rules")
    .description("Discover, author, and validate scoped engineering invariants");

  rulesCmd
    .command("list")
    .description("List all rules in .keel/rules/")
    .option("--json", "Output JSON")
    .action((opts: { json?: boolean }) => {
      const root = resolveRepoRoot();
      const entries = listRules(root);
      if (opts.json) {
        printJson(entries);
        return;
      }
      if (entries.length === 0) {
        console.log("No rules found in .keel/rules/.");
        return;
      }
      for (const r of entries) {
        const scope = r.global ? "global" : r.paths.join(", ");
        console.log(`[${r.severity}] ${r.id} (${r.file})`);
        if (r.summary) console.log(`  ${r.summary}`);
        console.log(`  scope: ${scope}`);
      }
    });

  rulesCmd
    .command("match")
    .description(
      "Find rules matching file paths or current git changes (--git / --staged / --base)",
    )
    .argument("[paths...]", "Target file paths (relative to repo root)")
    .option("--git", "Match against all uncommitted git changes")
    .option("--staged", "Match against staged git changes")
    .option("--base <ref>", "Match against files changed since git ref")
    .option("--no-global", "Exclude global rules without path filters")
    .option("--json", "Output JSON")
    .action(
      (
        targetPaths: string[],
        opts: {
          git?: boolean;
          staged?: boolean;
          base?: string;
          global?: boolean;
          json?: boolean;
        },
      ) => {
        const root = resolveRepoRoot();
        const combined = new Set<string>(targetPaths);

        if (opts.git || opts.staged || opts.base) {
          for (const f of getGitChangedFiles(root, {
            staged: opts.staged,
            baseRef: opts.base,
          })) {
            combined.add(f);
          }
        }

        const targets = [...combined];
        const matched = matchRules(root, targets, {
          includeGlobal: opts.global !== false,
        });

        if (opts.json) {
          printJson({ targets, matched });
          return;
        }
        if (matched.length === 0) {
          console.log("No matching rules.");
          return;
        }
        for (const r of matched) {
          console.log(`[${r.severity}] ${r.id} -> ${r.file}`);
          if (r.summary) console.log(`  ${r.summary}`);
        }
      },
    );

  rulesCmd
    .command("add")
    .description("Create a scoped invariant, guideline, or ADR in .keel/rules/")
    .argument("<id>", "Short kebab-case rule identifier")
    .requiredOption("-s, --summary <text>", "One-line summary shown by match")
    .option(
      "--severity <level>",
      "Rule severity (invariant | guideline | adr)",
      "invariant",
    )
    .option("-p, --paths <glob...>", "POSIX glob pattern(s) scoping this rule")
    .option("-f, --force", "Overwrite if rule file already exists")
    .option("--json", "Output JSON")
    .action(
      (
        id: string,
        opts: {
          summary: string;
          severity?: string;
          paths?: string[];
          force?: boolean;
          json?: boolean;
        },
      ) => {
        const root = resolveRepoRoot();
        const entry = addRule(root, id, {
          summary: opts.summary,
          severity: opts.severity as RuleSeverity | undefined,
          paths: opts.paths,
          force: opts.force,
        });
        if (opts.json) {
          printJson(entry);
          return;
        }
        console.log(`Created rule [${entry.severity}] ${entry.id} -> ${entry.file}`);
      },
    );

  rulesCmd
    .command("check")
    .description("Validate frontmatter, IDs, and schema of all .keel/rules/ files")
    .option("--json", "Output JSON")
    .action((opts: { json?: boolean }) => {
      const root = resolveRepoRoot();
      const report = validateRules(root);
      if (opts.json) {
        printJson(report);
      } else if (report.valid) {
        console.log(`Rules OK (${report.checked} checked, 0 issues).`);
      } else {
        console.log(`Rule validation failed (${report.issues.length} issue(s)):`);
        for (const issue of report.issues) {
          console.log(`  - ${issue.file}: ${issue.message}`);
        }
      }
      if (!report.valid) {
        process.exitCode = 1;
      }
    });

  // keel task
  const taskCmd = program
    .command("task")
    .description("Manage lightweight task specs under .keel/tasks/");

  taskCmd
    .command("create")
    .description("Create a new task directory with spec.md and meta.json")
    .argument("<title>", "Task title")
    .option("--slug <slug>", "Custom slug for the task folder")
    .option("--json", "Output JSON")
    .action((title: string, opts: { slug?: string; json?: boolean }) => {
      const root = resolveRepoRoot();
      const record = createTask(root, title, { slug: opts.slug });
      if (opts.json) {
        printJson(record);
        return;
      }
      console.log(`Created task ${record.id}`);
      console.log(`  spec: ${record.specFile}`);
    });

  taskCmd
    .command("list")
    .description("List tasks under .keel/tasks/")
    .option("--all", "Include archived tasks")
    .option("--status <status>", "Filter by status (open | done)")
    .option("--json", "Output JSON")
    .action((opts: { all?: boolean; status?: string; json?: boolean }) => {
      const root = resolveRepoRoot();
      const status =
        opts.status === "open" || opts.status === "done"
          ? opts.status
          : undefined;
      const records = listTasks(root, {
        status,
        includeArchived: Boolean(opts.all),
      });
      if (opts.json) {
        printJson(records);
        return;
      }
      if (records.length === 0) {
        console.log("No tasks found.");
        return;
      }
      for (const t of records) {
        console.log(`[${t.status}] ${t.id} — ${t.title} (${t.specFile})`);
      }
    });

  taskCmd
    .command("show")
    .description("Print a task's metadata and spec.md")
    .argument("<id>", "Task ID")
    .option("--json", "Output JSON")
    .action((id: string, opts: { json?: boolean }) => {
      const root = resolveRepoRoot();
      const record = getTask(root, id);
      const specBody = fs.readFileSync(
        path.join(root, record.specFile),
        "utf-8",
      );
      if (opts.json) {
        printJson({ ...record, spec: specBody });
        return;
      }
      console.log(specBody);
    });

  taskCmd
    .command("done")
    .description("Mark a task as done")
    .argument("<id>", "Task ID")
    .option("--json", "Output JSON")
    .action((id: string, opts: { json?: boolean }) => {
      const root = resolveRepoRoot();
      const record = setTaskStatus(root, id, "done");
      if (opts.json) {
        printJson(record);
        return;
      }
      console.log(`Marked ${record.id} as done.`);
    });

  taskCmd
    .command("reopen")
    .description("Reopen a completed task")
    .argument("<id>", "Task ID")
    .option("--json", "Output JSON")
    .action((id: string, opts: { json?: boolean }) => {
      const root = resolveRepoRoot();
      const record = setTaskStatus(root, id, "open");
      if (opts.json) {
        printJson(record);
        return;
      }
      console.log(`Reopened ${record.id}.`);
    });

  taskCmd
    .command("archive")
    .description("Mark a task done and move it to .keel/tasks/archive/")
    .argument("<id>", "Task ID")
    .option("--json", "Output JSON")
    .action((id: string, opts: { json?: boolean }) => {
      const root = resolveRepoRoot();
      const record = archiveTask(root, id);
      if (opts.json) {
        printJson(record);
        return;
      }
      console.log(`Archived ${record.id} -> ${record.dir}`);
    });

  // keel verify
  program
    .command("verify")
    .description("Run deterministic verification gates from .keel/config.yaml")
    .option("-g, --gate <name...>", "Run specific gate(s) by name")
    .option("--fail-fast", "Stop running remaining gates after first failure")
    .option("--json", "Output JSON")
    .action(
      (opts: { gate?: string[]; failFast?: boolean; json?: boolean }) => {
        const root = resolveRepoRoot();
        const report = runVerify(root, {
          gateNames: opts.gate,
          failFast: opts.failFast,
        });
        if (opts.json) {
          printJson(report);
        } else if (report.gates.length === 0) {
          console.log(
            "No verification gates configured in .keel/config.yaml yet.",
          );
        } else {
          for (const g of report.gates) {
            const icon = g.passed ? "PASS" : "FAIL";
            console.log(
              `[${icon}] ${g.name} (${g.durationMs}ms) — ${g.command}`,
            );
            if (!g.passed && g.output) {
              console.log(g.output);
            }
          }
        }
        if (!report.passed) {
          process.exitCode = 1;
        }
      },
    );

  // keel mem
  const memCmd = program
    .command("mem")
    .description(
      "Search and inspect local AI coding sessions and repository specs",
    );

  memCmd
    .command("sources")
    .description("List discovered memory sources and session counts")
    .option("--source <name>", "Filter to a specific source")
    .option("--json", "Output JSON")
    .action((opts: { source?: string; json?: boolean }) => {
      const root = resolveRepoRoot();
      const stats = listMemorySources(root, { source: opts.source });
      if (opts.json) {
        printJson(stats);
        return;
      }
      for (const s of stats) {
        const locs = s.locations.length > 0 ? s.locations.join(", ") : "(none)";
        console.log(`${s.source}: ${s.sessionCount} session(s) — ${locs}`);
      }
    });

  memCmd
    .command("sessions")
    .description("List recent sessions across local AI tools and .keel/")
    .option("--source <name>", "Filter by source (keel | claude | codex | opencode | custom)")
    .option("-n, --limit <count>", "Max sessions to return", "20")
    .option("--json", "Output JSON")
    .action((opts: { source?: string; limit?: string; json?: boolean }) => {
      const root = resolveRepoRoot();
      const sessions = listMemorySessions(root, {
        source: opts.source,
        limit: parsePositiveInt(opts.limit, 20),
      });
      if (opts.json) {
        printJson(sessions);
        return;
      }
      if (sessions.length === 0) {
        console.log("No sessions found.");
        return;
      }
      for (const s of sessions) {
        console.log(`[${s.source}] ${s.id} (${s.turnCount} turns) — ${s.title}`);
      }
    });

  memCmd
    .command("search")
    .description("Search sessions and repo artifacts by keywords")
    .argument("<query>", "Search query")
    .option("--source <name>", "Filter by source")
    .option("-n, --limit <count>", "Max hits to return", "15")
    .option("--json", "Output JSON")
    .action(
      (
        query: string,
        opts: { source?: string; limit?: string; json?: boolean },
      ) => {
        const root = resolveRepoRoot();
        const hits = searchMemory(root, query, {
          source: opts.source,
          limit: parsePositiveInt(opts.limit, 15),
        });
        if (opts.json) {
          printJson(hits);
          return;
        }
        if (hits.length === 0) {
          console.log("No matches found.");
          return;
        }
        for (const h of hits) {
          console.log(
            `[${h.source}/${h.role}] ${h.sessionId} (score ${h.score})`,
          );
          console.log(`  ${h.snippet}`);
        }
      },
    );

  memCmd
    .command("show")
    .description("Show turns from a specific memory session")
    .argument("<sessionId>", "Session ID (full or suffix)")
    .option("-n, --limit <count>", "Max turns to print", "30")
    .option("--json", "Output JSON")
    .action(
      (sessionId: string, opts: { limit?: string; json?: boolean }) => {
        const root = resolveRepoRoot();
        const detail = getMemorySession(root, sessionId);
        const maxTurns = parsePositiveInt(opts.limit, 30);
        const turns = detail.turns.slice(0, maxTurns);
        if (opts.json) {
          printJson({ summary: detail.summary, turns });
          return;
        }
        console.log(`# ${detail.summary.title} (${detail.summary.id})`);
        for (const turn of turns) {
          console.log(`\n[${turn.role}]`);
          console.log(turn.text);
        }
      },
    );

  // keel ablate & restore
  const ablateCmd = program
    .command("ablate")
    .description(
      "Temporarily remove Keel surfaces into a verified external snapshot for A/B testing",
    )
    .option("--dry-run", "Preview files that would be ablated without modifying disk")
    .option("--state-dir <dir>", "Override external snapshot directory")
    .option("--json", "Output JSON")
    .action(
      (opts: { dryRun?: boolean; stateDir?: string; json?: boolean }) => {
        const root = resolveRepoRoot();
        const res = ablateRepo(root, {
          dryRun: opts.dryRun,
          stateDir: opts.stateDir,
        });
        if (opts.json) {
          printJson(res);
          return;
        }
        const verb = res.dryRun ? "Would ablate" : "Ablated";
        console.log(`${verb} ${res.entries.length} Keel surface(s):`);
        for (const e of res.entries) {
          console.log(`  - ${e.relativePath} (${e.kind})`);
        }
        if (!res.dryRun) {
          console.log(`Snapshot saved at: ${res.snapshotDir}`);
          console.log("Run `keel restore` to recover the exact prior state.");
        }
      },
    );

  ablateCmd
    .command("status")
    .description("Check whether the current repository is ablated")
    .option("--state-dir <dir>", "Override external snapshot directory")
    .option("--json", "Output JSON")
    .action((opts: { stateDir?: string; json?: boolean }) => {
      const root = resolveRepoRoot();
      const status = getAblateStatus(root, opts.stateDir);
      if (opts.json) {
        printJson(status);
        return;
      }
      if (status.ablated) {
        console.log(
          `Repository is ABLATED (${status.entryCount} entries, since ${status.createdAt}).`,
        );
        console.log(`Snapshot: ${status.snapshotDir}`);
      } else {
        console.log("Repository is not ablated.");
      }
    });

  program
    .command("restore")
    .description("Restore Keel surfaces from the verified external ablation snapshot")
    .option("--dry-run", "Preview restoration and check for conflicts without writing")
    .option("-f, --force", "Overwrite paths even if modified while ablated")
    .option("--state-dir <dir>", "Override external snapshot directory")
    .option("--json", "Output JSON")
    .action(
      (opts: {
        dryRun?: boolean;
        force?: boolean;
        stateDir?: string;
        json?: boolean;
      }) => {
        const root = resolveRepoRoot();
        const res = restoreRepo(root, {
          dryRun: opts.dryRun,
          force: opts.force,
          stateDir: opts.stateDir,
        });
        if (opts.json) {
          printJson(res);
          return;
        }
        const verb = res.dryRun ? "Would restore" : "Restored";
        console.log(`${verb} ${res.restored.length} file(s):`);
        for (const f of res.restored) {
          console.log(`  + ${f}`);
        }
      },
    );

  // keel mcp
  program
    .command("mcp")
    .description("Start Keel's built-in stdio MCP server for the current repository")
    .action(async () => {
      const root = resolveRepoRoot();
      await startMcpStdioServer(root);
    });

  return program;
}

export async function runCli(argv: string[]): Promise<void> {
  const program = createProgram();
  await program.parseAsync(argv);
}
