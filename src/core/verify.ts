import { spawnSync } from "node:child_process";
import path from "node:path";
import { loadConfig } from "./config.js";
import { getGitChangedFiles, type GitChangedOptions } from "./git.js";
import { matchesAnyPath } from "./glob.js";
import type { VerifyGateResult, VerifyReport } from "./types.js";

export interface RunVerifyOptions extends GitChangedOptions {
  git?: boolean;
  gateNames?: string[];
  failFast?: boolean;
}

export function runVerify(
  repoRoot: string,
  options: RunVerifyOptions = {},
): VerifyReport {
  const config = loadConfig(repoRoot);
  const requested =
    options.gateNames?.filter((n) => n.trim().length > 0) ?? [];

  let gatesToRun =
    requested.length > 0
      ? config.verify.gates.filter((g) => requested.includes(g.name))
      : config.verify.gates;

  let skipped: string[] | undefined;
  if (requested.length > 0) {
    const foundNames = new Set(gatesToRun.map((g) => g.name));
    const missing = requested.filter((name) => !foundNames.has(name));
    if (missing.length > 0) {
      throw new Error(`Unknown verification gate(s): ${missing.join(", ")}`);
    }
  } else if (options.git || options.staged || options.baseRef) {
    const changes = getGitChangedFiles(repoRoot, options);
    const unmatched: string[] = [];
    gatesToRun = gatesToRun.filter((gate) => {
      if (!gate.paths?.length || gate.paths.some((pattern) => matchesAnyPath(pattern, changes))) return true;
      unmatched.push(gate.name);
      return false;
    });
    skipped = unmatched;
  }

  const results: VerifyGateResult[] = [];

  for (const gate of gatesToRun) {
    const start = Date.now();
    const cwd = gate.cwd ? path.resolve(repoRoot, gate.cwd) : repoRoot;
    const proc = spawnSync(gate.command, {
      cwd,
      shell: true,
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    const durationMs = Date.now() - start;
    const exitCode = proc.status ?? 1;
    const combinedOutput = [proc.stdout, proc.stderr]
      .filter((s) => typeof s === "string" && s.length > 0)
      .join("\n")
      .trim();

    const passed = exitCode === 0;
    results.push({
      name: gate.name,
      command: gate.command,
      passed,
      exitCode,
      durationMs,
      output: combinedOutput,
    });

    if (!passed && options.failFast) {
      break;
    }
  }

  return {
    passed: results.every((r) => r.passed),
    gates: results,
    ...(skipped ? { skipped } : {}),
  };
}
