import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { toPosixPath } from "./paths.js";

export interface Hotspot {
  file: string;
  score: number;
  touches: number;
  fixTouches: number;
  recentFixes: Array<{ hash: string; subject: string }>;
}

const fixMarker = /\b(?:fix|bug|revert|hotfix|patch)\b/i;

export function gitHotspots(repoRoot: string, commits = 100, limit = 10): Hotspot[] {
  if (!Number.isSafeInteger(commits) || !Number.isSafeInteger(limit) || commits < 1 || limit < 1) return [];
  const log = spawnSync("git", ["log", `-n${commits}`, "--no-color", "--no-decorate", "--no-notes", "--format=format:%x00%H%x00%s%x00", "--name-only", "-z", "--"], {
    cwd: repoRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: Infinity,
  });
  if (log.status !== 0 || !log.stdout) return [];
  const fields = log.stdout.split("\0");
  const files = new Map<string, Hotspot>();
  let index = 0;
  while (index < fields.length) {
    while (fields[index] === "") index++;
    const hash = fields[index++];
    const subject = fields[index++];
    if (!hash || subject === undefined) break;
    const isFix = fixMarker.test(subject);
    const touched = new Set<string>();
    let first = true;
    while (index < fields.length && fields[index] !== "") {
      const field = fields[index++]!;
      const file = toPosixPath(first ? field.replace(/^\n/, "") : field);
      first = false;
      if (file) touched.add(file);
    }
    for (const file of touched) {
      const item = files.get(file) ?? { file, score: 0, touches: 0, fixTouches: 0, recentFixes: [] };
      item.touches++;
      if (isFix) {
        item.fixTouches++;
        if (item.recentFixes.length < 3) item.recentFixes.push({ hash: hash.slice(0, 8), subject });
      }
      item.score = item.fixTouches * 3 + item.touches;
      files.set(file, item);
    }
  }
  return [...files.values()].filter((item) => fs.statSync(path.join(repoRoot, item.file), { throwIfNoEntry: false })?.isFile())
    .sort((left, right) => right.score - left.score || (left.file < right.file ? -1 : 1)).slice(0, limit);
}
