import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { toPosixPath } from "./paths.js";
import type { AblateManifest, AblateManifestEntry } from "./types.js";

type Operation = "ablate" | "migrate";

export interface AblateOptions { dryRun?: boolean; stateDir?: string }
export interface RestoreOptions extends AblateOptions { force?: boolean }
export interface AblateResult {
  dryRun: boolean;
  snapshotDir: string;
  entries: AblateManifestEntry[];
  directories?: string[];
}
export interface RestoreResult { dryRun: boolean; restored: string[] }

export interface Mutation {
  file: string;
  original?: Buffer;
  replacement: Buffer | null;
}

function hash(bytes: Buffer): string {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function isWithin(root: string, target: string): boolean {
  const rel = path.relative(root, target);
  return rel === "" || (!path.isAbsolute(rel) && rel !== ".." && !rel.startsWith(`..${path.sep}`));
}

function physicalPath(target: string): string {
  const stat = fs.lstatSync(target, { throwIfNoEntry: false });
  if (stat) return fs.realpathSync(target);
  return path.join(physicalPath(path.dirname(target)), path.basename(target));
}

export function resolveSnapshotDir(root: string, stateDir?: string, operation: Operation = "ablate"): string {
  const realRoot = physicalPath(path.resolve(root));
  const base = physicalPath(path.resolve(stateDir ?? path.join(os.homedir(), `.keel-${operation}`)));
  const dir = path.join(base, hash(Buffer.from(realRoot)).slice(0, 16));
  if (isWithin(realRoot, dir) || isWithin(dir, realRoot) || fs.lstatSync(dir, { throwIfNoEntry: false })?.isSymbolicLink()) {
    throw new Error("Snapshot directory must be outside the repository and must not be a symbolic link.");
  }
  return dir;
}

export function snapshotPath(root: string, relative: string): string {
  const parts = relative.split("/");
  if (relative.includes("\\") || relative.includes("\0") || parts.some((part) => !part || part === "." || part === ".." || /^[a-z]:/i.test(part))) {
    throw new Error(`Invalid snapshot path: ${relative}`);
  }
  let target = root;
  for (const [index, part] of parts.entries()) {
    target = path.join(target, part);
    const stat = fs.lstatSync(target, { throwIfNoEntry: false });
    if (stat?.isSymbolicLink()) {
      throw new Error(`Refusing symbolic link in snapshot path: ${relative}`);
    }
    if (stat && index < parts.length - 1 && !stat.isDirectory()) throw new Error(`Not a directory in snapshot path: ${relative}`);
  }
  return target;
}

function currentHash(file: string): string | null {
  const stat = fs.lstatSync(file, { throwIfNoEntry: false });
  if (!stat) return null;
  if (!stat.isFile()) throw new Error(`Not a regular file: ${file}`);
  return hash(fs.readFileSync(file));
}

function replaceFile(file: string, bytes: Buffer, mode?: number): void {
  const temp = path.join(path.dirname(file), `.keel-${crypto.randomUUID()}.tmp`);
  const permissions = mode ?? fs.lstatSync(file, { throwIfNoEntry: false })?.mode;
  const fd = fs.openSync(temp, "wx", permissions === undefined ? 0o666 : permissions & 0o777);
  try {
    try { fs.writeFileSync(fd, bytes); } finally { fs.closeSync(fd); }
    if (permissions !== undefined && process.platform !== "win32") fs.chmodSync(temp, permissions & 0o777);
    fs.renameSync(temp, file);
  } finally {
    if (fs.existsSync(temp)) fs.unlinkSync(temp);
  }
}

function stripEmptyParents(dir: string, root: string): void {
  const stop = path.resolve(root);
  for (let cur = path.resolve(dir); cur !== stop && isWithin(stop, cur); cur = path.dirname(cur)) {
    if (!fs.existsSync(cur) || fs.readdirSync(cur).length) break;
    fs.rmdirSync(cur);
  }
}

export function applySnapshot(root: string, operation: Operation, changes: Mutation[], options: AblateOptions, directories: string[] = []): AblateResult {
  const snapshotDir = resolveSnapshotDir(root, options.stateDir, operation);
  if (fs.existsSync(snapshotDir)) {
    throw new Error(operation === "ablate" ? `Repository is already ablated (snapshot: ${snapshotDir}). Run \`keel restore\` first.` : `Migration snapshot already exists: ${snapshotDir}. Roll back before cleaning again.`);
  }
  if (!changes.length && !directories.length && operation === "ablate") throw new Error("No Keel surfaces found to ablate in this repository.");
  const entries: AblateManifestEntry[] = changes.map(({ file, original, replacement }) => {
    const full = snapshotPath(root, file);
    const originalSha256 = original ? hash(original) : currentHash(full);
    if (originalSha256 === null) throw new Error(`Missing file during snapshot planning: ${file}`);
    return {
      kind: replacement === null ? "file" : file === "AGENTS.md" ? "agents-block" : "config-edit",
      relativePath: file, backupRelPath: file, originalSha256,
      postAblateSha256: replacement === null ? null : hash(replacement),
      ...(operation === "migrate" ? { originalMode: fs.statSync(full).mode & 0o777 } : {}),
    };
  });
  const result = { dryRun: !!options.dryRun, snapshotDir, entries, ...(directories.length ? { directories } : {}) };
  for (const dir of directories) snapshotPath(root, dir);
  if (options.dryRun || (!entries.length && !directories.length)) return result;

  fs.mkdirSync(path.dirname(snapshotDir), { recursive: true, mode: 0o700 });
  fs.mkdirSync(snapshotDir, { mode: 0o700 });
  const manifest: AblateManifest = {
    version: 1, repoRoot: path.resolve(root), createdAt: new Date().toISOString(), entries,
    ...(operation === "migrate" ? { operation } : {}), ...(directories.length ? { directories } : {}),
  };
  try {
    for (const [index, entry] of entries.entries()) {
      const target = snapshotPath(snapshotDir, `files/${entry.backupRelPath}`);
      fs.mkdirSync(path.dirname(target), { recursive: true, mode: 0o700 });
      const original = changes[index]!.original;
      if (original) fs.writeFileSync(target, original, { flag: "wx", mode: 0o600 });
      else {
        fs.copyFileSync(snapshotPath(root, entry.relativePath), target, fs.constants.COPYFILE_EXCL);
        if (process.platform !== "win32") fs.chmodSync(target, 0o600);
      }
      if (currentHash(target) !== entry.originalSha256) throw new Error(`Backup verification failed for ${entry.relativePath}.`);
    }
    for (const entry of entries) {
      if (currentHash(snapshotPath(root, entry.relativePath)) !== entry.originalSha256) throw new Error(`File changed during backup: ${entry.relativePath}`);
    }
    fs.writeFileSync(path.join(snapshotDir, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  } catch (error) {
    fs.rmSync(snapshotDir, { recursive: true, force: true });
    throw error;
  }

  for (const change of changes) {
    const full = snapshotPath(root, change.file);
    if (change.replacement !== null) replaceFile(full, change.replacement);
    else {
      fs.unlinkSync(full);
      if (operation === "ablate") stripEmptyParents(path.dirname(full), root);
    }
  }
  for (const dir of [...directories].sort((left, right) => right.length - left.length)) {
    fs.rmdirSync(snapshotPath(root, dir));
  }
  return result;
}

export function restoreSnapshot(root: string, operation: Operation, options: RestoreOptions): RestoreResult {
  const snapshotDir = resolveSnapshotDir(root, options.stateDir, operation);
  const file = snapshotPath(snapshotDir, "manifest.json");
  if (!fs.existsSync(file)) throw new Error(`No active ${operation === "ablate" ? "ablation" : "migration"} snapshot found for this repository (checked ${snapshotDir}).`);
  const manifest = JSON.parse(fs.readFileSync(file, "utf8")) as AblateManifest;
  if (manifest.version !== 1 || (manifest.operation ?? "ablate") !== operation ||
      physicalPath(path.resolve(manifest.repoRoot)) !== physicalPath(path.resolve(root)) || !Array.isArray(manifest.entries)) {
    throw new Error("Snapshot manifest does not match this repository or operation.");
  }
  const directories = manifest.directories ?? [];
  const conflicts: string[] = [];
  for (const relative of directories) {
    const dir = snapshotPath(root, relative);
    const stat = fs.lstatSync(dir, { throwIfNoEntry: false });
    if (stat && !stat.isDirectory()) throw new Error(`Cannot restore directory over a file: ${relative}`);
    if (stat) conflicts.push(relative);
  }
  const files = manifest.entries.map((entry) => {
    const target = snapshotPath(root, entry.relativePath);
    if (currentHash(target) !== entry.postAblateSha256) conflicts.push(entry.relativePath);
    const backup = snapshotPath(snapshotDir, `files/${entry.backupRelPath}`);
    if (!fs.existsSync(backup)) throw new Error(`Missing backup file in snapshot: ${entry.backupRelPath}`);
    const bytes = fs.readFileSync(backup);
    if (hash(bytes) !== entry.originalSha256) throw new Error(`Snapshot integrity check failed for ${entry.relativePath}.`);
    return { target, bytes, mode: entry.originalMode };
  });
  if (conflicts.length && !options.force) {
    throw new Error(`Restore aborted because managed path(s) changed while ${operation === "ablate" ? "ablated" : "migrated"}: ${conflicts.join(", ")}. Re-run with --force to overwrite.`);
  }
  if (!options.dryRun) {
    for (const dir of directories) fs.mkdirSync(snapshotPath(root, dir), { recursive: true });
    for (const { target, bytes, mode } of files) {
      fs.mkdirSync(path.dirname(target), { recursive: true });
      replaceFile(target, bytes, mode);
    }
    fs.rmSync(snapshotDir, { recursive: true });
  }
  return { dryRun: !!options.dryRun, restored: manifest.entries.map((entry) => entry.relativePath) };
}

export function collectSnapshotTree(root: string, relative: string, changes: Mutation[], directories?: string[]): void {
  const full = snapshotPath(root, relative);
  if (!fs.existsSync(full)) return;
  directories?.push(relative);
  for (const entry of fs.readdirSync(full, { withFileTypes: true })) {
    const file = `${relative}/${entry.name}`;
    if (entry.isDirectory()) collectSnapshotTree(root, file, changes, directories);
    else if (entry.isFile()) changes.push({ file: toPosixPath(file), replacement: null });
    else if (directories) throw new Error(`Unsupported entry in migration tree: ${file}`);
  }
}
