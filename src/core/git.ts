import { spawnSync } from "node:child_process";
import { toPosixPath } from "./paths.js";

export interface GitChangedOptions {
  staged?: boolean;
  baseRef?: string;
}

/**
 * Return repository-relative POSIX paths changed in git.
 * Combines unstaged, staged, and untracked files by default.
 */
export function getGitChangedFiles(
  repoRoot: string,
  options: GitChangedOptions = {},
): string[] {
  const files = new Set<string>();

  const runGit = (args: string[]): string[] => {
    const proc = spawnSync("git", args, {
      cwd: repoRoot,
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "pipe"],
      maxBuffer: Infinity,
    });
    if (proc.status !== 0 || typeof proc.stdout !== "string") {
      return [];
    }
    return proc.stdout
      .split("\0")
      .filter((line) => line.length > 0)
      .map((line) => toPosixPath(line));
  };

  if (options.baseRef) {
    for (const f of runGit(["diff", "--name-only", "-z", "--end-of-options", `${options.baseRef}...HEAD`, "--"])) {
      files.add(f);
    }
    for (const f of runGit(["diff", "--name-only", "-z"])) {
      files.add(f);
    }
    return [...files].sort();
  }

  if (options.staged) {
    for (const f of runGit(["diff", "--cached", "--name-only", "-z"])) {
      files.add(f);
    }
    return [...files].sort();
  }

  for (const f of runGit(["diff", "--name-only", "-z"])) files.add(f);
  for (const f of runGit(["diff", "--cached", "--name-only", "-z"])) files.add(f);
  for (const f of runGit(["ls-files", "--others", "--exclude-standard", "-z"])) {
    files.add(f);
  }

  return [...files].sort();
}
