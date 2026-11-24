import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { runVerify } from "../src/core/verify.js";

const tempDirs: string[] = [];

function makeTempRepo(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "keel-verify-"));
  tempDirs.push(dir);
  fs.mkdirSync(path.join(dir, ".keel"), { recursive: true });
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("verify gates", () => {
  it("runs configured gates and reports pass/fail accurately", () => {
    const root = makeTempRepo();
    fs.writeFileSync(
      path.join(root, ".keel", "config.yaml"),
      `version: 1
verify:
  gates:
    - name: ok-gate
      command: node -e "console.log('ok')"
    - name: fail-gate
      command: node -e "console.error('boom'); process.exit(2)"
`,
      "utf-8",
    );

    const okOnly = runVerify(root, { gateNames: ["ok-gate"] });
    expect(okOnly.passed).toBe(true);
    expect(okOnly.gates).toHaveLength(1);
    expect(okOnly.gates[0]?.output).toBe("ok");

    const all = runVerify(root);
    expect(all.passed).toBe(false);
    expect(all.gates).toHaveLength(2);
    expect(all.gates[1]?.passed).toBe(false);
    expect(all.gates[1]?.exitCode).toBe(2);
    expect(all.gates[1]?.output).toContain("boom");
  });
});
