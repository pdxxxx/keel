import { toPosixPath } from "./paths.js";

/**
 * Convert a POSIX glob pattern into a RegExp.
 * Supports single-star, double-star directory globs, and `?` wildcards.
 */
export function globToRegExp(pattern: string): RegExp {
  const normalized = toPosixPath(pattern).replace(/^\/+/, "");
  let regex = "^";
  let i = 0;

  while (i < normalized.length) {
    const ch = normalized[i]!;

    if (ch === "*") {
      if (normalized[i + 1] === "*") {
        // Consume `**/` as zero-or-more directories, or trailing `**` as anything.
        if (normalized[i + 2] === "/") {
          regex += "(?:.*/)?";
          i += 3;
        } else {
          regex += ".*";
          i += 2;
        }
      } else {
        regex += "[^/]*";
        i += 1;
      }
      continue;
    }

    if (ch === "?") {
      regex += "[^/]";
      i += 1;
      continue;
    }

    // Escape regex metacharacters
    if (/[.+^${}()|[\]\\]/.test(ch)) {
      regex += `\\${ch}`;
    } else {
      regex += ch;
    }
    i += 1;
  }

  regex += "$";
  return new RegExp(regex);
}

export function matchGlob(pattern: string, targetPath: string): boolean {
  const normalizedTarget = toPosixPath(targetPath).replace(/^\/+/, "");
  return globToRegExp(pattern).test(normalizedTarget);
}
