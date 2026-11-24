import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  ablateRepo,
  getAblateStatus,
  restoreRepo,
} from "../src/core/ablate.js";
import { initRepo } from "../src/core/init.js";

const tempDirs: string[] = [];

function makeTempDir(prefix: string): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("ablate & restore", () => {
  it("ablates Keel surfaces while preserving user AGENTS.md prose and restores byte-identically", () => {
    const repo = makeTempDir("keel-ablate-repo-");
    const stateDir = makeTempDir("keel-ablate-state-");

    const agentsPath = path.join(repo, "AGENTS.md");
    fs.writeFileSync(agentsPath, "# Team Notes\n\nAlways write tests.\n", "utf-8");
    initRepo(repo);

    const beforeAgents = fs.readFileSync(agentsPath, "utf-8");
    const beforeConfig = fs.readFileSync(
      path.join(repo, ".keel", "config.yaml"),
      "utf-8",
    );

    const dry = ablateRepo(repo, { dryRun: true, stateDir });
    expect(dry.dryRun).toBe(true);
    expect(fs.existsSync(path.join(repo, ".keel", "config.yaml"))).toBe(true);

    const ablated = ablateRepo(repo, { stateDir });
    expect(ablated.dryRun).toBe(false);
    expect(fs.existsSync(path.join(repo, ".keel"))).toBe(false);
    expect(fs.existsSync(path.join(repo, ".agents", "skills", "keel-plan"))).toBe(false);

    // User prose in AGENTS.md remains intact, managed block removed.
    const duringAgents = fs.readFileSync(agentsPath, "utf-8");
    expect(duringAgents).toContain("# Team Notes");
    expect(duringAgents).not.toContain("<!-- KEEL:START -->");

    expect(getAblateStatus(repo, stateDir).ablated).toBe(true);

    const restored = restoreRepo(repo, { stateDir });
    expect(restored.restored.length).toBeGreaterThan(0);
    expect(fs.readFileSync(agentsPath, "utf-8")).toBe(beforeAgents);
    expect(
      fs.readFileSync(path.join(repo, ".keel", "config.yaml"), "utf-8"),
    ).toBe(beforeConfig);
    expect(getAblateStatus(repo, stateDir).ablated).toBe(false);
  });

  it("refuses restore on conflict unless --force is passed", () => {
    const repo = makeTempDir("keel-ablate-conflict-");
    const stateDir = makeTempDir("keel-ablate-state2-");
    initRepo(repo);
    ablateRepo(repo, { stateDir });

    // Mutate an ablated path while ablated.
    fs.mkdirSync(path.join(repo, ".keel"), { recursive: true });
    fs.writeFileSync(path.join(repo, ".keel", "config.yaml"), "conflict: true\n");

    expect(() => restoreRepo(repo, { stateDir })).toThrow(/changed while ablated/);
    expect(getAblateStatus(repo, stateDir).ablated).toBe(true);

    const forced = restoreRepo(repo, { stateDir, force: true });
    expect(forced.restored).toContain(".keel/config.yaml");
    expect(getAblateStatus(repo, stateDir).ablated).toBe(false);
  });
});
