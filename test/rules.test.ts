import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  addRule,
  listRules,
  matchRules,
  validateRules,
} from "../src/core/rules.js";

const tempDirs: string[] = [];

function makeTempRepo(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "keel-rules-"));
  tempDirs.push(dir);
  fs.mkdirSync(path.join(dir, ".keel", "rules"), { recursive: true });
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("rules engine", () => {
  it.each([
    { name: "LF", newline: "\n", after: "\n# Body" },
    { name: "CRLF", newline: "\r\n", after: "\r\n# Body" },
    { name: "EOF", newline: "\n", after: "" },
  ])("keeps delimiter-like YAML content up to a standalone $name delimiter", ({ newline, after }) => {
    const root = makeTempRepo();
    const rule = [
      "---", "id: boundary", "---foo: harmless", "summary: |-",
      "  before", "  ---foo", "  after", "severity: adr", "---",
    ].join(newline) + after;
    fs.writeFileSync(path.join(root, ".keel", "rules", "boundary.md"), rule);

    expect(listRules(root)).toMatchObject([{ id: "boundary", summary: "before\n---foo\nafter", severity: "adr" }]);
    expect(validateRules(root)).toEqual({ valid: true, checked: 1, issues: [] });
  });

  it("rejects closing lines with trailing characters, spaces or a bare CR", () => {
    const root = makeTempRepo();
    const file = path.join(root, ".keel", "rules", "boundary.md");
    for (const closing of ["---foo", "--- \n", "---\r", "---\r# Body", "  ---\n"]) {
      fs.writeFileSync(file, `---\nid: boundary\nsummary: kept\n${closing}`);
      expect(validateRules(root).issues).toEqual([{
        file: ".keel/rules/boundary.md",
        message: "Missing YAML frontmatter (expected id, summary, severity).",
      }]);
    }
  });

  it("recognizes an empty header with adjacent standalone delimiters", () => {
    const root = makeTempRepo();
    fs.writeFileSync(path.join(root, ".keel", "rules", "empty.md"), "---\r\n---\r\n# Body");
    expect(listRules(root)[0]?.summary).toBe("Body");
    expect(validateRules(root).issues.map((issue) => issue.message)).toEqual([
      "Missing non-empty `id`.", "Missing non-empty `summary`.",
    ]);
  });

  it("adds, lists, validates, and matches scoped and global rules", () => {
    const root = makeTempRepo();
    addRule(root, "auth-rotation", {
      summary: "Rotate refresh tokens atomically",
      severity: "invariant",
      paths: ["src/auth/**"],
    });
    addRule(root, "global-errors", {
      summary: "Never swallow async errors silently",
      severity: "guideline",
    });

    const validation = validateRules(root);
    expect(validation.valid).toBe(true);
    expect(validation.checked).toBe(2);

    const all = listRules(root);
    expect(all).toHaveLength(2);

    const authMatch = matchRules(root, ["src/auth/session/refresh.ts"]);
    expect(authMatch.map((r) => r.id)).toEqual(["auth-rotation", "global-errors"]);

    const uiMatch = matchRules(root, ["src/ui/button.tsx"]);
    expect(uiMatch.map((r) => r.id)).toEqual(["global-errors"]);

    const uiScopedOnly = matchRules(root, ["src/ui/button.tsx"], {
      includeGlobal: false,
    });
    expect(uiScopedOnly).toHaveLength(0);
  });

  it("reports duplicate IDs and invalid severities during validation", () => {
    const root = makeTempRepo();
    fs.writeFileSync(
      path.join(root, ".keel", "rules", "a.md"),
      `---\nid: dup-id\nsummary: First\nseverity: invariant\n---\nBody\n`,
    );
    fs.writeFileSync(
      path.join(root, ".keel", "rules", "b.md"),
      `---\nid: dup-id\nsummary: Second\nseverity: bogus\n---\nBody\n`,
    );

    const report = validateRules(root);
    expect(report.valid).toBe(false);
    expect(report.issues.some((i) => i.message.includes("Duplicate rule id"))).toBe(true);
    expect(report.issues.some((i) => i.message.includes("Invalid severity"))).toBe(true);
  });
});
