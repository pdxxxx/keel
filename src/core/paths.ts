import fs from "node:fs";
import path from "node:path";

export const KEEL_DIR = ".keel";
export const CONFIG_FILE = "config.yaml";
export const RULES_DIR = "rules";
export const TASKS_DIR = "tasks";
export const ARCHIVE_DIR = "archive";
export const AGENTS_FILE = "AGENTS.md";
export const SKILLS_DIR = path.join(".agents", "skills");

export function toPosixPath(filePath: string): string {
  return filePath.replace(/\\/g, "/").replace(/^\.\//, "");
}

/**
 * Walk upward from `startDir` to locate the nearest directory containing `.keel/` or `.git/`.
 * Falls back to `startDir` when neither marker exists.
 */
export function resolveRepoRoot(startDir: string = process.cwd()): string {
  const resolved = path.resolve(startDir);
  let cur = resolved;

  while (true) {
    if (fs.existsSync(path.join(cur, KEEL_DIR))) {
      return cur;
    }
    const parent = path.dirname(cur);
    if (parent === cur) break;
    cur = parent;
  }

  cur = resolved;
  while (true) {
    if (fs.existsSync(path.join(cur, ".git"))) {
      return cur;
    }
    const parent = path.dirname(cur);
    if (parent === cur) break;
    cur = parent;
  }

  return resolved;
}

export function keelPath(root: string, ...segments: string[]): string {
  return path.join(root, KEEL_DIR, ...segments);
}
