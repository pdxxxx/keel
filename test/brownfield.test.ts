import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { gitHotspots } from "../src/core/hotspots.js";
import { validateRules } from "../src/core/rules.js";
import { runVerify } from "../src/core/verify.js";
import { cli, fixture, git, put, revision } from "./fixtures/brownfield.js";

const gates = [
  { name: "api", command: "echo api", paths: ["./packages\\api\\**"] },
  { name: "ui", command: "echo ui", paths: ["packages/ui/**"] },
  { name: "global", command: "echo global" },
  { name: "empty", command: "echo empty", paths: [] },
];
const gateNames = (report: ReturnType<typeof runVerify>) => report.gates.map((gate) => gate.name);

describe("scoped verification", () => {
  it("preserves full verification, runs only globals on a clean tree, and ignores paths for explicit gates", () => {
    const { root } = fixture();
    git(root, "init", "-q");
    put(root, ".keel/config.yaml", JSON.stringify({ verify: { gates } }));
    revision(root, "initial");
    const full = runVerify(root);
    expect(gateNames(full)).toEqual(["api", "ui", "global", "empty"]);
    expect(full).not.toHaveProperty("skipped");
    const clean = runVerify(root, { git: true, gateNames: [] });
    expect(gateNames(clean)).toEqual(["global", "empty"]);
    expect(clean.skipped).toEqual(["api", "ui"]);
    expect(gateNames(runVerify(root, { git: true, gateNames: ["ui"] }))).toEqual(["ui"]);
    expect(() => runVerify(root, { git: true, gateNames: ["unknown"] })).toThrow("Unknown verification gate(s): unknown");
    put(root, ".keel/config.yaml", JSON.stringify({ verify: { gates: gates.slice(0, 2) } }));
    expect(runVerify(root, { git: true })).toEqual({ passed: true, gates: [], skipped: ["api", "ui"] });
  });

  it("uses actual staged, unstaged and untracked names including non-ASCII", () => {
    const { root } = fixture();
    git(root, "init", "-q");
    put(root, ".keel/config.yaml", JSON.stringify({ verify: { gates } }));
    put(root, "packages/api/索引.ts", "original");
    revision(root, "initial");
    put(root, "packages/api/索引.ts", "staged");
    git(root, "add", "packages/api/索引.ts");
    put(root, "packages/ui/new file.ts", "untracked");
    expect(gateNames(runVerify(root, { staged: true }))).toEqual(["api", "global", "empty"]);
    expect(gateNames(runVerify(root, { git: true }))).toEqual(["api", "ui", "global", "empty"]);
  });

  it("exposes --git, --staged, --base and named overrides through the CLI", () => {
    const { root } = fixture();
    git(root, "init", "-q");
    put(root, ".keel/config.yaml", JSON.stringify({ verify: { gates } }));
    const base = revision(root, "initial");
    put(root, "packages/api/version", "new");
    revision(root, "change api", base);
    put(root, "packages/ui/index", "staged");
    git(root, "add", "packages/ui/index");
    const run = (...args: string[]) => {
      const proc = cli(root, "verify", ...args, "--json");
      expect(proc.status, proc.stderr).toBe(0);
      return gateNames(JSON.parse(proc.stdout));
    };
    expect(run("--git")).toEqual(["ui", "global", "empty"]);
    expect(run("--staged")).toEqual(["ui", "global", "empty"]);
    expect(run("--git", "--gate", "api")).toEqual(["api"]);
    expect(run("--base", base)).toContain("api");
  });
});

describe("rule hygiene", () => {
  it("keeps legacy reports unchanged and checks individual globs against real, non-generated files", () => {
    const { root } = fixture();
    put(root, ".keel/rules/a.md", "---\nid: a\nsummary: scoped\npaths: ['src/**/?.ts', 'old/**', '**/generated']\n---\n");
    put(root, "src/sub/文.ts", "");
    for (const dir of [".git", "node_modules", "packages/api/target", "dist"]) put(root, `${dir}/generated`, "");
    expect(validateRules(root)).toEqual({ valid: true, checked: 1, issues: [] });
    const stale = validateRules(root, { stale: true });
    expect(stale.valid).toBe(true);
    expect(stale.warnings?.map((warning) => warning.message)).toEqual([
      "Path glob `old/**` matches no repository files.", "Path glob `**/generated` matches no repository files.",
    ]);
    put(root, "old/generated", "");
    expect(validateRules(root, { stale: true }).warnings).toEqual([]);
  });

  it("warns only above the 80-line and five-global-rule thresholds, excluding the index", () => {
    const { root } = fixture();
    const rule = (id: string, lines: number) => [`---`, `id: ${id}`, "summary: text", "---", ...Array(lines - 4).fill("body")].join("\r\n") + "\r\n";
    for (let index = 0; index < 5; index++) put(root, `.keel/rules/${index}.md`, rule(`global-${index}`, 80));
    put(root, ".keel/rules/index.md", "line\n".repeat(100));
    expect(validateRules(root, { stale: true }).warnings).toEqual([]);
    put(root, ".keel/rules/extra.md", rule("extra", 81));
    expect(validateRules(root, { stale: true }).warnings).toEqual([
      { file: ".keel/rules/extra.md", message: "Rule has 81 lines (recommended maximum: 80)." },
      { file: ".keel/rules", message: "6 global rules (recommended maximum: 5)." },
    ]);
  });

  it("CLI separates schema validity from the --strict failure policy", () => {
    const { root } = fixture();
    put(root, ".keel/rules/orphan.md", "---\nid: old\nsummary: Old\npaths: [missing/**]\n---\n");
    const normal = cli(root, "rules", "check", "--json");
    expect(JSON.parse(normal.stdout)).not.toHaveProperty("warnings");
    const stale = cli(root, "rules", "check", "--stale", "--json");
    expect(stale.status).toBe(0);
    const strict = cli(root, "rules", "check", "--strict", "--json");
    expect(strict.status).toBe(1);
    expect(JSON.parse(strict.stdout)).toEqual(JSON.parse(stale.stdout));
    put(root, "missing/file", "present");
    expect(cli(root, "rules", "check", "--stale", "--strict").status).toBe(0);
  });
});

describe("Git hotspots", () => {
  it("ranks unique commit touches, recognizes whole-word markers, caps recent fixes and omits deleted files", () => {
    const { root } = fixture();
    git(root, "init", "-q");
    const file = "lib/修复 file.ts";
    put(root, file, "0"); put(root, "stable", "text"); put(root, "removed", "soon");
    let parent = revision(root, "prefix only");
    const fixes = [];
    for (const subject of ["fix: first", "repair a BUG", "hotfix(api)", "Revert old change", "PATCH release"]) {
      put(root, file, subject);
      parent = revision(root, subject, parent);
      fixes.unshift({ hash: parent.slice(0, 8), subject });
    }
    fs.unlinkSync(path.join(root, "removed"));
    expect(gitHotspots(root)).toEqual([
      { file, touches: 6, fixTouches: 5, score: 21, recentFixes: fixes.slice(0, 3) },
      { file: "stable", touches: 1, fixTouches: 0, score: 1, recentFixes: [] },
    ]);
    const result = cli(root, "hotspots", "--commits", "2", "--limit", "1", "--json");
    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toEqual([{ file, touches: 2, fixTouches: 2, score: 8, recentFixes: fixes.slice(0, 2) }]);
  });

  it("handles non-Git and unborn repositories without failing", () => {
    const { root } = fixture();
    expect(gitHotspots(root)).toEqual([]);
    git(root, "init", "-q");
    expect(gitHotspots(root)).toEqual([]);
    expect(cli(root, "hotspots", "--json").stdout.trim()).toBe("[]");
  });
});
