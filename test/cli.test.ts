import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";

const tempDirs: string[] = [];
const binPath = path.resolve("bin/keel.js");

function makeTempDir(prefix: string): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

function runKeel(cwd: string, args: string[]) {
  return spawnSync(process.execPath, [binPath, ...args], {
    cwd,
    encoding: "utf-8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("CLI end-to-end", () => {
  it("runs init, rules, task, mem, verify, ablate, and restore cleanly", () => {
    const repo = makeTempDir("keel-e2e-repo-");
    const stateDir = makeTempDir("keel-e2e-state-");

    expect(runKeel(repo, ["--version"]).stdout.trim()).toBe("1.1.0");
    expect(runKeel(repo, ["init"]).status).toBe(0);

    const addRes = runKeel(repo, [
      "rules",
      "add",
      "no-float-money",
      "--summary",
      "Store monetary amounts as integer minor units",
      "--paths",
      "src/payments/**",
    ]);
    expect(addRes.status).toBe(0);

    expect(runKeel(repo, ["rules", "check"]).status).toBe(0);

    const matchRes = runKeel(repo, [
      "rules",
      "match",
      "src/payments/invoice.ts",
      "--json",
    ]);
    expect(matchRes.status).toBe(0);
    expect(matchRes.stdout).toContain("no-float-money");

    const taskRes = runKeel(repo, [
      "task",
      "create",
      "Fix currency rounding",
      "--slug",
      "currency-rounding",
      "--json",
    ]);
    expect(taskRes.status).toBe(0);

    const memRes = runKeel(repo, [
      "mem",
      "search",
      "monetary amounts",
      "--source",
      "keel",
      "--json",
    ]);
    expect(memRes.status).toBe(0);
    expect(memRes.stdout).toContain("no-float-money");

    expect(runKeel(repo, ["verify"]).status).toBe(0);

    expect(runKeel(repo, ["ablate", "--state-dir", stateDir]).status).toBe(0);
    expect(fs.existsSync(path.join(repo, ".keel"))).toBe(false);

    expect(runKeel(repo, ["restore", "--state-dir", stateDir]).status).toBe(0);
    expect(fs.existsSync(path.join(repo, ".keel", "rules", "no-float-money.md"))).toBe(true);
  });
});
