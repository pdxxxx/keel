import fs from "node:fs";
import path from "node:path";
import { AGENTS_FILE, KEEL_DIR, SKILLS_DIR, toPosixPath } from "./paths.js";
import { AGENTS_END_MARKER, AGENTS_START_MARKER } from "../templates/assets.js";
import type { AblateManifest } from "./types.js";
import {
  applySnapshot, collectSnapshotTree, resolveSnapshotDir, restoreSnapshot, snapshotPath,
  type AblateOptions, type AblateResult, type Mutation, type RestoreOptions, type RestoreResult,
} from "./snapshot.js";

export type { AblateOptions, AblateResult, RestoreOptions, RestoreResult } from "./snapshot.js";

export interface AblateStatus {
  ablated: boolean;
  snapshotDir: string;
  createdAt?: string;
  entryCount: number;
}

function stripManagedAgentsBlock(content: string): { found: boolean; stripped: string } {
  const startIdx = content.indexOf(AGENTS_START_MARKER);
  const endIdx = content.indexOf(AGENTS_END_MARKER);
  if (startIdx === -1 || endIdx === -1 || endIdx <= startIdx) return { found: false, stripped: content };
  const before = content.slice(0, startIdx).trimEnd();
  const after = content.slice(endIdx + AGENTS_END_MARKER.length).trimStart();
  if (!before && !after) return { found: true, stripped: "" };
  if (!before) return { found: true, stripped: `${after}\n` };
  if (!after) return { found: true, stripped: `${before}\n` };
  return { found: true, stripped: `${before}\n\n${after}\n` };
}

export function getAblateStatus(repoRoot: string, stateDir?: string): AblateStatus {
  const snapshotDir = resolveSnapshotDir(repoRoot, stateDir);
  const manifestPath = snapshotPath(snapshotDir, "manifest.json");
  if (!fs.existsSync(manifestPath)) return { ablated: false, snapshotDir, entryCount: 0 };
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as AblateManifest;
  if (manifest.operation) return { ablated: false, snapshotDir, entryCount: 0 };
  return { ablated: true, snapshotDir, createdAt: manifest.createdAt, entryCount: manifest.entries.length };
}

export function ablateRepo(repoRoot: string, options: AblateOptions = {}): AblateResult {
  const changes: Mutation[] = [];
  collectSnapshotTree(repoRoot, KEEL_DIR, changes);
  const skillsRoot = snapshotPath(repoRoot, toPosixPath(SKILLS_DIR));
  if (fs.existsSync(skillsRoot)) {
    for (const entry of fs.readdirSync(skillsRoot, { withFileTypes: true })) {
      if (entry.isDirectory() && entry.name.startsWith("keel-")) {
        collectSnapshotTree(repoRoot, toPosixPath(path.join(SKILLS_DIR, entry.name)), changes);
      }
    }
  }
  const agents = snapshotPath(repoRoot, AGENTS_FILE);
  if (fs.existsSync(agents)) {
    const original = fs.readFileSync(agents);
    const { found, stripped } = stripManagedAgentsBlock(original.toString("utf8"));
    if (found) changes.push({ file: AGENTS_FILE, original, replacement: stripped ? Buffer.from(stripped) : null });
  }
  return applySnapshot(repoRoot, "ablate", changes, options);
}

export function restoreRepo(repoRoot: string, options: RestoreOptions = {}): RestoreResult {
  return restoreSnapshot(repoRoot, "ablate", options);
}
