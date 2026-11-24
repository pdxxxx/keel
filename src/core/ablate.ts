import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  AGENTS_FILE,
  KEEL_DIR,
  SKILLS_DIR,
  toPosixPath,
} from "./paths.js";
import {
  AGENTS_END_MARKER,
  AGENTS_START_MARKER,
} from "../templates/assets.js";
import type { AblateManifest, AblateManifestEntry } from "./types.js";

export interface AblateOptions {
  dryRun?: boolean;
  stateDir?: string;
}

export interface RestoreOptions {
  dryRun?: boolean;
  force?: boolean;
  stateDir?: string;
}

export interface AblateStatus {
  ablated: boolean;
  snapshotDir: string;
  createdAt?: string;
  entryCount: number;
}

export interface AblateResult {
  dryRun: boolean;
  snapshotDir: string;
  entries: AblateManifestEntry[];
}

export interface RestoreResult {
  dryRun: boolean;
  restored: string[];
}

function sha256Buffer(buf: Buffer): string {
  return crypto.createHash("sha256").update(buf).digest("hex");
}

function sha256File(filePath: string): string | null {
  if (!fs.existsSync(filePath)) return null;
  return sha256Buffer(fs.readFileSync(filePath));
}

function resolveSnapshotDir(repoRoot: string, stateDir?: string): string {
  const realRoot = fs.existsSync(repoRoot)
    ? fs.realpathSync(repoRoot)
    : path.resolve(repoRoot);
  const hash = crypto
    .createHash("sha256")
    .update(realRoot)
    .digest("hex")
    .slice(0, 16);
  const base = stateDir ?? path.join(os.homedir(), ".keel-ablate");
  return path.join(base, hash);
}

function collectFilesRecursively(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  const files: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectFilesRecursively(full));
    } else if (entry.isFile()) {
      files.push(full);
    }
  }
  return files.sort();
}

function stripEmptyParents(dir: string, stopAt: string): void {
  let cur = path.resolve(dir);
  const stop = path.resolve(stopAt);
  while (cur !== stop && cur.startsWith(stop)) {
    if (!fs.existsSync(cur)) {
      cur = path.dirname(cur);
      continue;
    }
    const items = fs.readdirSync(cur);
    if (items.length > 0) break;
    fs.rmdirSync(cur);
    cur = path.dirname(cur);
  }
}

function stripManagedAgentsBlock(content: string): {
  found: boolean;
  stripped: string;
} {
  const startIdx = content.indexOf(AGENTS_START_MARKER);
  const endIdx = content.indexOf(AGENTS_END_MARKER);
  if (startIdx === -1 || endIdx === -1 || endIdx <= startIdx) {
    return { found: false, stripped: content };
  }

  const before = content.slice(0, startIdx).trimEnd();
  const after = content.slice(endIdx + AGENTS_END_MARKER.length).trimStart();

  if (!before && !after) {
    return { found: true, stripped: "" };
  }
  if (!before) {
    return { found: true, stripped: `${after}\n` };
  }
  if (!after) {
    return { found: true, stripped: `${before}\n` };
  }
  return { found: true, stripped: `${before}\n\n${after}\n` };
}

export function getAblateStatus(
  repoRoot: string,
  stateDir?: string,
): AblateStatus {
  const snapshotDir = resolveSnapshotDir(repoRoot, stateDir);
  const manifestPath = path.join(snapshotDir, "manifest.json");
  if (!fs.existsSync(manifestPath)) {
    return { ablated: false, snapshotDir, entryCount: 0 };
  }

  const manifest = JSON.parse(
    fs.readFileSync(manifestPath, "utf-8"),
  ) as AblateManifest;
  return {
    ablated: true,
    snapshotDir,
    createdAt: manifest.createdAt,
    entryCount: manifest.entries.length,
  };
}

export function ablateRepo(
  repoRoot: string,
  options: AblateOptions = {},
): AblateResult {
  const snapshotDir = resolveSnapshotDir(repoRoot, options.stateDir);
  const manifestPath = path.join(snapshotDir, "manifest.json");
  if (fs.existsSync(manifestPath)) {
    throw new Error(
      `Repository is already ablated (snapshot: ${snapshotDir}). Run \`keel restore\` first.`,
    );
  }

  const planned: Array<{
    entry: AblateManifestEntry;
    fullPath: string;
    originalBytes: Buffer;
    strippedBytes: Buffer | null;
  }> = [];

  // 1. All files under .keel/
  const keelDir = path.join(repoRoot, KEEL_DIR);
  for (const fullPath of collectFilesRecursively(keelDir)) {
    const rel = toPosixPath(path.relative(repoRoot, fullPath));
    const originalBytes = fs.readFileSync(fullPath);
    planned.push({
      fullPath,
      originalBytes,
      strippedBytes: null,
      entry: {
        kind: "file",
        relativePath: rel,
        originalSha256: sha256Buffer(originalBytes),
        postAblateSha256: null,
        backupRelPath: rel,
      },
    });
  }

  // 2. All Keel-managed skills under .agents/skills/keel-*
  const skillsRoot = path.join(repoRoot, SKILLS_DIR);
  if (fs.existsSync(skillsRoot)) {
    for (const dirEntry of fs.readdirSync(skillsRoot, { withFileTypes: true })) {
      if (!dirEntry.isDirectory() || !dirEntry.name.startsWith("keel-")) {
        continue;
      }
      const skillDir = path.join(skillsRoot, dirEntry.name);
      for (const fullPath of collectFilesRecursively(skillDir)) {
        const rel = toPosixPath(path.relative(repoRoot, fullPath));
        const originalBytes = fs.readFileSync(fullPath);
        planned.push({
          fullPath,
          originalBytes,
          strippedBytes: null,
          entry: {
            kind: "file",
            relativePath: rel,
            originalSha256: sha256Buffer(originalBytes),
            postAblateSha256: null,
            backupRelPath: rel,
          },
        });
      }
    }
  }

  // 3. Managed block in AGENTS.md
  const agentsPath = path.join(repoRoot, AGENTS_FILE);
  if (fs.existsSync(agentsPath)) {
    const originalBytes = fs.readFileSync(agentsPath);
    const { found, stripped } = stripManagedAgentsBlock(
      originalBytes.toString("utf-8"),
    );
    if (found) {
      const strippedBytes =
        stripped.length > 0 ? Buffer.from(stripped, "utf-8") : null;
      planned.push({
        fullPath: agentsPath,
        originalBytes,
        strippedBytes,
        entry: {
          kind: strippedBytes ? "agents-block" : "file",
          relativePath: AGENTS_FILE,
          originalSha256: sha256Buffer(originalBytes),
          postAblateSha256: strippedBytes ? sha256Buffer(strippedBytes) : null,
          backupRelPath: AGENTS_FILE,
        },
      });
    }
  }

  if (planned.length === 0) {
    throw new Error("No Keel surfaces found to ablate in this repository.");
  }

  if (options.dryRun) {
    return {
      dryRun: true,
      snapshotDir,
      entries: planned.map((p) => p.entry),
    };
  }

  // Write verified backups before touching the repository.
  const filesBackupRoot = path.join(snapshotDir, "files");
  fs.mkdirSync(filesBackupRoot, { recursive: true });

  for (const item of planned) {
    const backupTarget = path.join(filesBackupRoot, item.entry.backupRelPath);
    fs.mkdirSync(path.dirname(backupTarget), { recursive: true });
    fs.writeFileSync(backupTarget, item.originalBytes);
    const verified = sha256File(backupTarget);
    if (verified !== item.entry.originalSha256) {
      throw new Error(
        `Backup verification failed for ${item.entry.relativePath}; aborting ablation.`,
      );
    }
  }

  const manifest: AblateManifest = {
    version: 1,
    repoRoot: path.resolve(repoRoot),
    createdAt: new Date().toISOString(),
    entries: planned.map((p) => p.entry),
  };
  fs.writeFileSync(
    manifestPath,
    `${JSON.stringify(manifest, null, 2)}\n`,
    "utf-8",
  );

  // Now apply ablation mutations to the workspace.
  for (const item of planned) {
    if (item.strippedBytes) {
      fs.writeFileSync(item.fullPath, item.strippedBytes);
    } else if (fs.existsSync(item.fullPath)) {
      fs.unlinkSync(item.fullPath);
      stripEmptyParents(path.dirname(item.fullPath), repoRoot);
    }
  }

  return {
    dryRun: false,
    snapshotDir,
    entries: manifest.entries,
  };
}

export function restoreRepo(
  repoRoot: string,
  options: RestoreOptions = {},
): RestoreResult {
  const snapshotDir = resolveSnapshotDir(repoRoot, options.stateDir);
  const manifestPath = path.join(snapshotDir, "manifest.json");
  if (!fs.existsSync(manifestPath)) {
    throw new Error(
      `No active ablation snapshot found for this repository (checked ${snapshotDir}).`,
    );
  }

  const manifest = JSON.parse(
    fs.readFileSync(manifestPath, "utf-8"),
  ) as AblateManifest;
  const filesBackupRoot = path.join(snapshotDir, "files");

  // Conflict check before writing anything.
  const conflicts: string[] = [];
  for (const entry of manifest.entries) {
    const targetPath = path.join(repoRoot, entry.relativePath);
    const currentHash = sha256File(targetPath);
    if (currentHash !== entry.postAblateSha256) {
      conflicts.push(entry.relativePath);
    }
  }

  if (conflicts.length > 0 && !options.force) {
    throw new Error(
      `Restore aborted because managed path(s) changed while ablated: ${conflicts.join(", ")}. Re-run with --force to overwrite.`,
    );
  }

  if (options.dryRun) {
    return {
      dryRun: true,
      restored: manifest.entries.map((e) => e.relativePath),
    };
  }

  const restored: string[] = [];
  for (const entry of manifest.entries) {
    const backupPath = path.join(filesBackupRoot, entry.backupRelPath);
    if (!fs.existsSync(backupPath)) {
      throw new Error(`Missing backup file in snapshot: ${entry.backupRelPath}`);
    }
    const bytes = fs.readFileSync(backupPath);
    if (sha256Buffer(bytes) !== entry.originalSha256) {
      throw new Error(
        `Snapshot integrity check failed for ${entry.relativePath}.`,
      );
    }

    const targetPath = path.join(repoRoot, entry.relativePath);
    fs.mkdirSync(path.dirname(targetPath), { recursive: true });
    fs.writeFileSync(targetPath, bytes);
    restored.push(entry.relativePath);
  }

  // Clean up snapshot only after every file restored cleanly.
  fs.rmSync(snapshotDir, { recursive: true, force: true });

  return {
    dryRun: false,
    restored,
  };
}
