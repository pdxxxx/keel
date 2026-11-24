import fs from "node:fs";
import YAML from "yaml";
import { CONFIG_FILE, keelPath } from "./paths.js";
import type { KeelConfig, VerifyGateConfig } from "./types.js";

export function createEmptyConfig(): KeelConfig {
  return {
    version: 1,
    verify: { gates: [] },
    memory: { sources: [] },
  };
}

export function loadConfig(repoRoot: string): KeelConfig {
  const filePath = keelPath(repoRoot, CONFIG_FILE);
  if (!fs.existsSync(filePath)) {
    return createEmptyConfig();
  }

  const raw = fs.readFileSync(filePath, "utf-8");
  const parsed = YAML.parse(raw) as Record<string, unknown> | null;
  if (!parsed || typeof parsed !== "object") {
    return createEmptyConfig();
  }

  const version = typeof parsed.version === "number" ? parsed.version : 1;

  const verifySection =
    parsed.verify && typeof parsed.verify === "object"
      ? (parsed.verify as Record<string, unknown>)
      : {};
  const rawGates = Array.isArray(verifySection.gates)
    ? verifySection.gates
    : [];

  const gates: VerifyGateConfig[] = [];
  for (const item of rawGates) {
    if (!item || typeof item !== "object") continue;
    const rec = item as Record<string, unknown>;
    if (typeof rec.name === "string" && typeof rec.command === "string") {
      gates.push({
        name: rec.name.trim(),
        command: rec.command.trim(),
        ...(typeof rec.cwd === "string" && rec.cwd.trim().length > 0
          ? { cwd: rec.cwd.trim() }
          : {}),
      });
    }
  }

  const memorySection =
    parsed.memory && typeof parsed.memory === "object"
      ? (parsed.memory as Record<string, unknown>)
      : {};
  const rawSources = Array.isArray(memorySection.sources)
    ? memorySection.sources
    : [];
  const sources = rawSources
    .filter((s): s is string => typeof s === "string" && s.trim().length > 0)
    .map((s) => s.trim());

  return {
    version,
    verify: { gates },
    memory: { sources },
  };
}
