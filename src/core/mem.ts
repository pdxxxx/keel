import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { loadConfig } from "./config.js";
import { keelPath, RULES_DIR, TASKS_DIR, toPosixPath } from "./paths.js";
import type {
  MemorySearchHit,
  MemorySessionSummary,
  MemoryTurn,
} from "./types.js";

export interface MemoryOptions {
  source?: string;
  limit?: number;
  homeDir?: string;
  extraDirs?: string[];
  includeExternal?: boolean;
}

export interface MemorySourceStats {
  source: string;
  sessionCount: number;
  locations: string[];
}

export interface MemorySessionDetail {
  summary: MemorySessionSummary;
  turns: MemoryTurn[];
}

interface RawSessionRecord {
  summary: MemorySessionSummary;
  turns: MemoryTurn[];
}

function stableId(prefix: string, filePath: string): string {
  const hash = crypto
    .createHash("sha256")
    .update(path.resolve(filePath))
    .digest("hex")
    .slice(0, 10);
  const base = path
    .basename(filePath, path.extname(filePath))
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .slice(0, 24);
  return `${prefix}:${base || "doc"}-${hash}`;
}

function walkFiles(
  dir: string,
  extensions: Set<string>,
  maxDepth = 5,
  depth = 0,
): string[] {
  if (depth > maxDepth || !fs.existsSync(dir)) return [];
  const files: string[] = [];

  let entries: fs.Dirent[];
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return [];
  }

  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(full, extensions, maxDepth, depth + 1));
    } else if (entry.isFile() && extensions.has(path.extname(entry.name))) {
      files.push(full);
    }
  }

  return files;
}

function extractTextFromContent(content: unknown): string {
  if (typeof content === "string") return content.trim();
  if (Array.isArray(content)) {
    const parts: string[] = [];
    for (const block of content) {
      if (typeof block === "string") {
        parts.push(block);
      } else if (block && typeof block === "object") {
        const rec = block as Record<string, unknown>;
        if (typeof rec.text === "string") {
          parts.push(rec.text);
        } else if (typeof rec.content === "string") {
          parts.push(rec.content);
        }
      }
    }
    return parts.join("\n").trim();
  }
  return "";
}

function parseJsonlTurns(filePath: string): MemoryTurn[] {
  let raw: string;
  try {
    raw = fs.readFileSync(filePath, "utf-8");
  } catch {
    return [];
  }

  const turns: MemoryTurn[] = [];
  for (const line of raw.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;

    let obj: Record<string, unknown>;
    try {
      const parsed = JSON.parse(trimmed);
      if (!parsed || typeof parsed !== "object") continue;
      obj = parsed as Record<string, unknown>;
    } catch {
      continue;
    }

    const timestamp =
      typeof obj.timestamp === "string"
        ? obj.timestamp
        : typeof obj.created_at === "string"
          ? obj.created_at
          : undefined;

    // Normalize common transcript shapes:
    // 1. { message: { role, content } } (Claude Code style)
    // 2. { payload: { role, content } } (Codex style)
    // 3. { role, content | text } (generic JSONL)
    const nested =
      obj.message && typeof obj.message === "object"
        ? (obj.message as Record<string, unknown>)
        : obj.payload && typeof obj.payload === "object"
          ? (obj.payload as Record<string, unknown>)
          : obj;

    const rawRole =
      typeof nested.role === "string"
        ? nested.role
        : typeof obj.type === "string"
          ? obj.type
          : "";

    let role: MemoryTurn["role"] | null = null;
    if (rawRole === "user" || rawRole === "human") {
      role = "user";
    } else if (rawRole === "assistant" || rawRole === "model") {
      role = "assistant";
    }

    if (!role) continue;

    const text =
      extractTextFromContent(nested.content) ||
      extractTextFromContent(nested.text) ||
      extractTextFromContent(obj.content) ||
      extractTextFromContent(obj.text);

    if (!text) continue;
    turns.push({ role, text, ...(timestamp ? { timestamp } : {}) });
  }

  return turns;
}

function parseMarkdownArtifact(filePath: string): MemoryTurn[] {
  try {
    const raw = fs.readFileSync(filePath, "utf-8").trim();
    if (!raw) return [];
    return [{ role: "artifact", text: raw }];
  } catch {
    return [];
  }
}

function deriveTitle(turns: MemoryTurn[], fallbackFile: string): string {
  const firstUser = turns.find((t) => t.role === "user") ?? turns[0];
  if (firstUser) {
    const firstLine = firstUser.text
      .split(/\r?\n/)
      .map((l) => l.trim())
      .find((l) => l.length > 0 && l !== "---");
    if (firstLine) {
      return firstLine.replace(/^#+\s*/, "").slice(0, 80);
    }
  }
  return path.basename(fallbackFile);
}

function collectSessionsFromDir(
  source: string,
  dir: string,
  extensions: Set<string>,
  displayRoot?: string,
): RawSessionRecord[] {
  const files = walkFiles(dir, extensions);
  const sessions: RawSessionRecord[] = [];

  for (const filePath of files) {
    const ext = path.extname(filePath);
    const turns =
      ext === ".jsonl"
        ? parseJsonlTurns(filePath)
        : parseMarkdownArtifact(filePath);
    if (turns.length === 0) continue;

    let mtimeIso = "";
    try {
      mtimeIso = fs.statSync(filePath).mtime.toISOString();
    } catch {
      mtimeIso = new Date(0).toISOString();
    }

    const displayFile = displayRoot
      ? toPosixPath(path.relative(displayRoot, filePath))
      : toPosixPath(filePath);

    sessions.push({
      summary: {
        id: stableId(source, filePath),
        source,
        file: displayFile,
        title: deriveTitle(turns, filePath),
        updatedAt: mtimeIso,
        turnCount: turns.length,
      },
      turns,
    });
  }

  return sessions;
}

function resolveSources(
  repoRoot: string,
  options: MemoryOptions = {},
): Array<{
  source: string;
  dirs: string[];
  extensions: Set<string>;
  displayRoot?: string;
}> {
  const home = options.homeDir ?? os.homedir();
  const includeExternal = options.includeExternal ?? true;
  const config = loadConfig(repoRoot);

  const list: Array<{
    source: string;
    dirs: string[];
    extensions: Set<string>;
    displayRoot?: string;
  }> = [
    {
      source: "keel",
      dirs: [keelPath(repoRoot, TASKS_DIR), keelPath(repoRoot, RULES_DIR)],
      extensions: new Set([".md"]),
      displayRoot: repoRoot,
    },
  ];

  if (includeExternal) {
    list.push(
      {
        source: "claude",
        dirs: [path.join(home, ".claude", "projects")],
        extensions: new Set([".jsonl"]),
      },
      {
        source: "codex",
        dirs: [path.join(home, ".codex", "sessions")],
        extensions: new Set([".jsonl"]),
      },
      {
        source: "opencode",
        dirs: [path.join(home, ".local", "share", "opencode")],
        extensions: new Set([".jsonl"]),
      },
    );
  }

  const envDirs = (process.env.KEEL_MEM_DIRS ?? "")
    .split(path.delimiter)
    .map((d) => d.trim())
    .filter((d) => d.length > 0);

  const customDirs = [
    ...config.memory.sources.map((d) => path.resolve(repoRoot, d)),
    ...envDirs.map((d) => path.resolve(d)),
    ...(options.extraDirs ?? []).map((d) => path.resolve(d)),
  ];

  if (customDirs.length > 0) {
    list.push({
      source: "custom",
      dirs: [...new Set(customDirs)],
      extensions: new Set([".jsonl", ".md"]),
    });
  }

  if (options.source) {
    return list.filter((item) => item.source === options.source);
  }

  return list;
}

function loadAllSessions(
  repoRoot: string,
  options: MemoryOptions = {},
): RawSessionRecord[] {
  const sources = resolveSources(repoRoot, options);
  const all: RawSessionRecord[] = [];

  for (const src of sources) {
    for (const dir of src.dirs) {
      all.push(
        ...collectSessionsFromDir(
          src.source,
          dir,
          src.extensions,
          src.displayRoot,
        ),
      );
    }
  }

  return all.sort((a, b) =>
    b.summary.updatedAt.localeCompare(a.summary.updatedAt),
  );
}

export function listMemorySources(
  repoRoot: string,
  options: MemoryOptions = {},
): MemorySourceStats[] {
  const sources = resolveSources(repoRoot, options);
  return sources.map((src) => {
    const existingDirs = src.dirs.filter((d) => fs.existsSync(d));
    let count = 0;
    for (const dir of existingDirs) {
      count += collectSessionsFromDir(
        src.source,
        dir,
        src.extensions,
        src.displayRoot,
      ).length;
    }
    return {
      source: src.source,
      sessionCount: count,
      locations: existingDirs.map((d) => toPosixPath(d)),
    };
  });
}

export function listMemorySessions(
  repoRoot: string,
  options: MemoryOptions = {},
): MemorySessionSummary[] {
  const limit = options.limit ?? 20;
  return loadAllSessions(repoRoot, options)
    .slice(0, Math.max(0, limit))
    .map((s) => s.summary);
}

export function getMemorySession(
  repoRoot: string,
  sessionId: string,
  options: MemoryOptions = {},
): MemorySessionDetail {
  const sessions = loadAllSessions(repoRoot, options);
  const found = sessions.find(
    (s) => s.summary.id === sessionId || s.summary.id.endsWith(sessionId),
  );
  if (!found) {
    throw new Error(`Memory session not found: ${sessionId}`);
  }
  return found;
}

function buildSnippet(text: string, terms: string[], radius = 120): string {
  const lower = text.toLowerCase();
  let firstIdx = -1;
  for (const term of terms) {
    const idx = lower.indexOf(term);
    if (idx !== -1 && (firstIdx === -1 || idx < firstIdx)) {
      firstIdx = idx;
    }
  }
  if (firstIdx === -1) {
    return text.slice(0, radius * 2).replace(/\s+/g, " ").trim();
  }
  const start = Math.max(0, firstIdx - radius);
  const end = Math.min(text.length, firstIdx + radius);
  const prefix = start > 0 ? "..." : "";
  const suffix = end < text.length ? "..." : "";
  return `${prefix}${text.slice(start, end).replace(/\s+/g, " ").trim()}${suffix}`;
}

export function searchMemory(
  repoRoot: string,
  query: string,
  options: MemoryOptions = {},
): MemorySearchHit[] {
  const terms = query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter((t) => t.length > 0);
  if (terms.length === 0) return [];

  const limit = options.limit ?? 15;
  const sessions = loadAllSessions(repoRoot, options);
  const hits: MemorySearchHit[] = [];

  for (const session of sessions) {
    for (const turn of session.turns) {
      const lower = turn.text.toLowerCase();
      let matchedTerms = 0;
      let totalOccurrences = 0;

      for (const term of terms) {
        let pos = 0;
        let termCount = 0;
        while ((pos = lower.indexOf(term, pos)) !== -1) {
          termCount += 1;
          pos += term.length;
        }
        if (termCount > 0) {
          matchedTerms += 1;
          totalOccurrences += termCount;
        }
      }

      if (matchedTerms === 0) continue;

      // Strongly reward matching all query terms, plus frequency bonus.
      const coverageRatio = matchedTerms / terms.length;
      const score = Number(
        (coverageRatio * 10 + Math.min(totalOccurrences, 10) * 0.5).toFixed(2),
      );

      hits.push({
        sessionId: session.summary.id,
        source: session.summary.source,
        file: session.summary.file,
        role: turn.role,
        score,
        snippet: buildSnippet(turn.text, terms),
        ...(turn.timestamp ? { timestamp: turn.timestamp } : {}),
      });
    }
  }

  return hits
    .sort((a, b) => b.score - a.score)
    .slice(0, Math.max(0, limit));
}
