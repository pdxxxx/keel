import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { onTestFinished } from "vitest";

export function fixture() {
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), "keel-v11-test-"));
  onTestFinished(() => fs.rmSync(folder, { recursive: true, force: true }));
  const root = path.join(folder, "repo");
  fs.mkdirSync(root);
  return { folder, root, stateDir: path.join(folder, "backups") };
}

export function put(root: string, file: string, body: string | Buffer): void {
  const full = path.join(root, file);
  fs.mkdirSync(path.dirname(full), { recursive: true });
  fs.writeFileSync(full, body);
}

export function git(root: string, ...args: string[]): string {
  return execFileSync("git", ["-c", "user.name=Keel Test", "-c", "user.email=fixture@example.invalid", ...args], {
    cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

export function revision(root: string, subject: string, parent?: string): string {
  git(root, "add", "-A");
  const tree = git(root, "write-tree");
  const hash = git(root, "commit-tree", tree, ...(parent ? ["-p", parent] : []), "-m", subject);
  git(root, "update-ref", "HEAD", hash);
  return hash;
}

const binary = path.resolve("bin/keel.js");
export function cli(root: string, ...args: string[]) {
  return spawnSync(process.execPath, [binary, ...args], { cwd: root, encoding: "utf8", timeout: 20000 });
}
