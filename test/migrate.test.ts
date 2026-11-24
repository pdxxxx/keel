import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { describe, expect, it, vi } from "vitest";
import { initRepo } from "../src/core/init.js";
import { cleanupMigration, planMigration, rollbackMigration, scanMigration } from "../src/core/migrate.js";
import { cli, fixture, put } from "./fixtures/brownfield.js";

const prose = "# Notes\r\nKeep this line.\r\n";
const keel = "<!-- KEEL:START -->\r\nKeep the Keel block.\r\n<!-- KEEL:END -->\r\n";
const legacy = "<!-- TRELLIS:START -->\r\nOld dispatch rules.\r\n<!-- TRELLIS:END -->\r\n";
const settings = {
  permissions: { allow: ["Edit"] },
  hooks: {
    PreToolUse: [{ matcher: "Edit", hooks: [{ type: "command", command: "run trellis-check" }, { type: "command", command: "run local-lint" }] }],
    Stop: [{ hooks: [{ type: "command", command: "run team-check" }] }],
  },
};

function legacyRepo() {
  const result = fixture();
  const { root } = result;
  const files: Record<string, string | Buffer> = {
    "AGENTS.md": prose + legacy + keel + legacy + "Tail without a newline",
    ".trellis/tasks/archive/old/spec.md": "Original archived decision",
    ".trellis/data.bin": Buffer.from([0, 255, 12]),
    ".trellis/spec/backend.md": "Legacy notes for distillation",
    ".cursorrules": "Legacy cursor instructions",
    ".cursor/rules/api.mdc": "---\nglobs: ['src/**/*.ts', 'api/**']\n---\nReview\n",
    ".cursor/rules/ui.mdc": "---\nglobs: 'ui/**/*.{ts,tsx}, css/**'\n---\nReview\n",
    ".claude/agents/trellis-review.md": "retire",
    ".claude/agents/team-review.md": "keep",
    ".claude/hooks/check.sh": "run .trellis/check",
    ".claude/hooks/team.sh": "run team-check",
    ".claude/settings.json": JSON.stringify(settings, null, 4) + "\r\n",
    ".cursor/commands/trellis-start.md": "retire",
    ".cursor/agents/trellis-plan.md": "retire",
    "CLAUDE.md": "Claude rules", ".github/copilot-instructions.md": "Copilot rules",
    ".windsurfrules": "Old rules", ".windsurf/rules/team/design.md": "Windsurf design",
    ".specify/memory/constitution.md": "Decisions", "_bmad/templates/workflow.md": "Human review only",
    "docs/adr/001.md": "ADR", "package.json": JSON.stringify({ scripts: { lint: "eslint ." } }),
    ".github/workflows/ci.yml": "name: ci", "Makefile": "test:\n\techo ok\n",
  };
  for (const [file, contents] of Object.entries(files)) put(root, file, contents);
  fs.mkdirSync(path.join(root, ".trellis/empty/deep"), { recursive: true });
  return { ...result, files };
}

describe("one-shot migration", () => {
  it("discovers all seven legacy setups, classifies conflicts and retains Cursor glob hints", () => {
    const { root } = legacyRepo();
    const { items } = scanMigration(root);
    expect(new Set(items.map((item) => item.framework))).toEqual(new Set(["trellis", "cursor", "claude", "copilot", "windsurf", "spec-kit", "bmad", "project"]));
    const categories = Object.fromEntries(items.map((item) => [item.path, item.category]));
    expect(categories).toMatchObject({ "AGENTS.md": "conflict", ".trellis": "conflict", ".cursorrules": "conflict", ".claude/settings.json": "conflict",
      ".claude/hooks/check.sh": "conflict", ".claude/agents/trellis-review.md": "conflict", ".cursor/commands/trellis-start.md": "conflict",
      ".cursor/agents/trellis-plan.md": "conflict", ".trellis/tasks/archive": "memory-source", "docs/adr": "memory-source",
      ".cursor/rules/api.mdc": "rule-source", "CLAUDE.md": "rule-source", ".github/copilot-instructions.md": "rule-source",
      ".windsurf/rules/team/design.md": "rule-source", ".windsurfrules": "rule-source", ".specify": "rule-source", "_bmad": "rule-source",
      "Makefile": "gate-source", "package.json": "gate-source", ".github/workflows/ci.yml": "gate-source" });
    expect(categories[".claude/hooks/team.sh"]).toBeUndefined();
    expect(categories[".claude/agents/team-review.md"]).toBeUndefined();
    expect(items.find((item) => item.path === ".cursor/rules/api.mdc")?.globs).toEqual(["src/**/*.ts", "api/**"]);
    expect(items.find((item) => item.path === ".cursor/rules/ui.mdc")?.globs).toEqual(["ui/**/*.{ts,tsx}", "css/**"]);
  });

  it("produces a read-only plan with detected gates, rule inputs and existing archives", () => {
    const { root, stateDir } = legacyRepo();
    const plan = planMigration(root);
    expect(plan.suggestedGates).toEqual([{ name: "lint", command: "npm run lint" }]);
    expect(plan.suggestedMemorySources).toEqual([".trellis/tasks/archive", "docs/adr"]);
    expect(plan.ruleSources.map((item) => item.path)).toEqual(expect.arrayContaining([".cursorrules", ".trellis/spec/backend.md", "_bmad/templates/workflow.md", ".specify/memory/constitution.md"]));
    expect(fs.existsSync(path.join(root, ".keel"))).toBe(false);
    expect(fs.existsSync(stateDir)).toBe(false);
  });

  it("removes indirect references to retired hook scripts without deleting unrelated settings or agent directories", () => {
    const { root, stateDir } = fixture();
    put(root, ".claude/hooks/check.py", "import trellis");
    put(root, ".claude/agents/trellis-a/AGENT.md", "retire");
    put(root, ".claude/agents/local/AGENT.md", "keep");
    const hooks = [{ command: 'python ".claude\\hooks\\check.py"' }, { command: "python .claude/hooks/check.py.bak" }];
    put(root, ".claude/settings.json", JSON.stringify({ hooks: { Stop: [{ hooks }] }, model: "keep" }));
    cleanupMigration(root, { stateDir });
    expect(JSON.parse(fs.readFileSync(path.join(root, ".claude/settings.json"), "utf8"))).toEqual({ hooks: { Stop: [{ hooks: [hooks[1]] }] }, model: "keep" });
    expect(fs.existsSync(path.join(root, ".claude/agents/trellis-a"))).toBe(false);
    expect(fs.existsSync(path.join(root, ".claude/agents/local/AGENT.md"))).toBe(true);
    rollbackMigration(root, { stateDir });
    expect(fs.readFileSync(path.join(root, ".claude/agents/trellis-a/AGENT.md"), "utf8")).toBe("retire");
  });

  it("does not modify hard-linked user files when stripping or restoring AGENTS.md", () => {
    const { root, folder, stateDir } = fixture();
    const agents = path.join(root, "AGENTS.md");
    put(root, "AGENTS.md", prose + legacy + keel);
    const outside = path.join(folder, "user-copy");
    fs.linkSync(agents, outside);
    cleanupMigration(root, { stateDir });
    expect(fs.readFileSync(outside, "utf8")).toBe(prose + legacy + keel);
    const after = path.join(folder, "post-cleanup-copy");
    fs.linkSync(agents, after);
    rollbackMigration(root, { stateDir });
    expect(fs.readFileSync(after, "utf8")).toBe(prose + keel);
    expect(fs.readFileSync(agents, "utf8")).toBe(prose + legacy + keel);
  });

  it("previews every mutation without writes and restores bytes, empty directories and executable bits", () => {
    const { root, stateDir, files } = legacyRepo();
    const hookFile = path.join(root, ".claude/hooks/check.sh");
    if (process.platform !== "win32") fs.chmodSync(hookFile, 0o751);
    const preview = cleanupMigration(root, { stateDir, dryRun: true });
    expect(preview.entries.map((entry) => entry.relativePath)).toContain(".trellis/data.bin");
    expect(preview.directories).toContain(".trellis/empty/deep");
    expect(fs.existsSync(stateDir)).toBe(false);
    expect(fs.readFileSync(hookFile, "utf8")).toBe(files[".claude/hooks/check.sh"]);
    const applied = cleanupMigration(root, { stateDir });
    expect(applied.entries).toEqual(preview.entries);
    expect(fs.existsSync(path.join(root, ".trellis"))).toBe(false);
    expect(fs.existsSync(path.join(root, ".cursorrules"))).toBe(false);
    expect(fs.readFileSync(path.join(root, "AGENTS.md"), "utf8")).toBe(prose + keel + "Tail without a newline");
    expect(JSON.parse(fs.readFileSync(path.join(root, ".claude/settings.json"), "utf8"))).toEqual({
      permissions: settings.permissions, hooks: { PreToolUse: [{ matcher: "Edit", hooks: [{ type: "command", command: "run local-lint" }] }], Stop: settings.hooks.Stop },
    });
    expect(fs.existsSync(path.join(root, ".claude/hooks/team.sh"))).toBe(true);
    for (const entry of applied.entries) {
      const saved = fs.readFileSync(path.join(applied.snapshotDir, "files", entry.backupRelPath));
      expect(crypto.createHash("sha256").update(saved).digest("hex")).toBe(entry.originalSha256);
      expect(saved.equals(Buffer.from(files[entry.relativePath]!))).toBe(true);
    }
    expect(() => cleanupMigration(root, { stateDir })).toThrow(/snapshot already exists/i);
    rollbackMigration(root, { stateDir });
    for (const [file, bytes] of Object.entries(files)) expect(fs.readFileSync(path.join(root, file))).toEqual(Buffer.from(bytes));
    expect(fs.existsSync(path.join(root, ".trellis/empty/deep"))).toBe(true);
    expect(fs.existsSync(applied.snapshotDir)).toBe(false);
    if (process.platform !== "win32") expect(fs.statSync(hookFile).mode & 0o777).toBe(0o751);
  });

  it("makes no workspace changes if any backup cannot be written", () => {
    const { root, stateDir, files } = legacyRepo();
    const write = fs.writeFileSync;
    const spy = vi.spyOn(fs, "writeFileSync").mockImplementation((target, data, options) => {
      if (String(target).startsWith(stateDir) && path.basename(String(target)) === "AGENTS.md") throw new Error("simulated disk full");
      write(target, data, options);
    });
    try { expect(() => cleanupMigration(root, { stateDir })).toThrow("simulated disk full"); }
    finally { spy.mockRestore(); }
    for (const [file, bytes] of Object.entries(files)) expect(fs.readFileSync(path.join(root, file))).toEqual(Buffer.from(bytes));
    expect(fs.readdirSync(stateDir)).toEqual([]);
  });

  it("rejects a corrupt late backup before restoring even the first file, including under --force", () => {
    const { root, stateDir } = legacyRepo();
    const cleaned = cleanupMigration(root, { stateDir });
    const last = cleaned.entries.at(-1)!;
    fs.writeFileSync(path.join(cleaned.snapshotDir, "files", last.backupRelPath), "bad backup");
    expect(() => rollbackMigration(root, { stateDir, force: true })).toThrow(/integrity check failed/);
    expect(fs.existsSync(path.join(root, ".cursorrules"))).toBe(false);
    expect(fs.existsSync(path.join(root, ".trellis"))).toBe(false);
    expect(fs.existsSync(cleaned.snapshotDir)).toBe(true);
  });

  it("rejects external snapshot misconfiguration and symlinked source trees without touching their targets", () => {
    const { root, folder, stateDir } = legacyRepo();
    expect(() => cleanupMigration(root, { stateDir: path.join(root, "bad-state") })).toThrow(/outside the repository/);
    expect(fs.existsSync(path.join(root, "bad-state"))).toBe(false);
    const outside = path.join(folder, "elsewhere");
    put(outside, "keep", "untouched");
    fs.symlinkSync(outside, path.join(root, ".trellis/linked"), process.platform === "win32" ? "junction" : "dir");
    expect(() => cleanupMigration(root, { stateDir })).toThrow(/Unsupported entry|symbolic link/);
    expect(fs.readFileSync(path.join(outside, "keep"), "utf8")).toBe("untouched");
    expect(fs.existsSync(stateDir)).toBe(false);
  });

  it("fails closed on unpaired or overlapping instruction blocks", () => {
    const { root, stateDir } = fixture();
    put(root, ".cursorrules", "keep");
    for (const contents of ["<!-- TRELLIS:START -->\nunfinished", legacy.replace("Old dispatch rules.", keel)]) {
      put(root, "AGENTS.md", contents);
      expect(() => cleanupMigration(root, { stateDir })).toThrow(/markers/i);
      expect(fs.readFileSync(path.join(root, "AGENTS.md"), "utf8")).toBe(contents);
      expect(fs.existsSync(path.join(root, ".cursorrules"))).toBe(true);
      expect(fs.existsSync(stateDir)).toBe(false);
    }
  });

  it("runs scan, plan, dry-run, cleanup and conflict-safe rollback via the real CLI", () => {
    const { root, stateDir, files } = legacyRepo();
    const run = (subcommand: string, ...args: string[]) => cli(root, "migrate", subcommand, ...args, "--json");
    expect(JSON.parse(run("scan").stdout)).toEqual(scanMigration(root));
    expect(JSON.parse(run("plan").stdout)).toEqual(planMigration(root));
    const state = ["--state-dir", stateDir];
    expect(JSON.parse(run("cleanup", ...state, "--dry-run").stdout).dryRun).toBe(true);
    expect(fs.existsSync(stateDir)).toBe(false);
    const applied = JSON.parse(run("cleanup", ...state).stdout);
    put(root, "AGENTS.md", "post-migration edit");
    const conflict = run("rollback", ...state);
    expect(conflict.status).not.toBe(0);
    expect(conflict.stderr).toContain("changed while migrated: AGENTS.md");
    expect(fs.existsSync(path.join(root, ".cursorrules"))).toBe(false);
    expect(fs.readFileSync(path.join(root, "AGENTS.md"), "utf8")).toBe("post-migration edit");
    expect(JSON.parse(run("rollback", ...state, "--force").stdout).restored).toContain("AGENTS.md");
    expect(fs.readFileSync(path.join(root, "AGENTS.md"), "utf8")).toBe(files["AGENTS.md"]);
    expect(fs.existsSync(applied.snapshotDir)).toBe(false);
  });

  it("installs the supplied migrate skill byte-for-byte and does not overwrite a locally edited skill", () => {
    const { root } = fixture();
    initRepo(root);
    const file = ".agents/skills/keel-migrate/SKILL.md";
    expect(fs.readFileSync(path.join(root, file))).toEqual(fs.readFileSync(file));
    put(root, file, "local skill");
    expect(initRepo(root).skipped).toContain(file);
  });
});
