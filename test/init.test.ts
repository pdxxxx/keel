import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { loadConfig } from "../src/core/config.js";
import { initRepo } from "../src/core/init.js";

const tempDirs: string[] = [];

function makeTempRepo(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "keel-init-"));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("initRepo", () => {
  it("creates .keel, AGENTS.md, and all four standard skills cleanly", () => {
    const root = makeTempRepo();
    const res = initRepo(root);

    expect(res.created).toContain(".keel/config.yaml");
    expect(res.created).toContain(".keel/rules/index.md");
    expect(res.created).toContain("AGENTS.md");
    expect(res.created).toContain(".agents/skills/keel-plan/SKILL.md");
    expect(res.created).toContain(".agents/skills/keel-verify/SKILL.md");
    expect(res.created).toContain(".agents/skills/keel-distill/SKILL.md");
    expect(res.created).toContain(".agents/skills/keel-mem/SKILL.md");
  });

  it("auto-detects pnpm scripts into .keel/config.yaml gates", () => {
    const root = makeTempRepo();
    fs.writeFileSync(path.join(root, "pnpm-lock.yaml"), "lockfileVersion: '9.0'\n");
    fs.writeFileSync(
      path.join(root, "package.json"),
      JSON.stringify({
        name: "demo",
        scripts: { typecheck: "tsc --noEmit", test: "vitest run" },
      }),
    );

    const res = initRepo(root);
    expect(res.detectedGates.map((g) => g.name)).toEqual(["typecheck", "test"]);

    const cfg = loadConfig(root);
    expect(cfg.verify.gates).toEqual([
      { name: "typecheck", command: "pnpm typecheck" },
      { name: "test", command: "pnpm test" },
    ]);
  });

  it("preserves user prose outside managed AGENTS.md markers on re-init", () => {
    const root = makeTempRepo();
    const agentsPath = path.join(root, "AGENTS.md");
    fs.writeFileSync(agentsPath, "# Custom Repo Notes\n\nKeep this.\n", "utf-8");

    const first = initRepo(root);
    expect(first.updated).toContain("AGENTS.md");
    const afterFirst = fs.readFileSync(agentsPath, "utf-8");
    expect(afterFirst).toContain("# Custom Repo Notes");
    expect(afterFirst).toContain("<!-- KEEL:START -->");
    expect(afterFirst).toContain("<!-- KEEL:END -->");

    const second = initRepo(root);
    expect(second.skipped).toContain("AGENTS.md");
    expect(fs.readFileSync(agentsPath, "utf-8")).toBe(afterFirst);
  });
});
