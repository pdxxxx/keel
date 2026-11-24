import fs from "node:fs";
import path from "node:path";
import { detectProjectGates } from "./init.js";
import { parseFrontmatter } from "./rules.js";
import { toPosixPath } from "./paths.js";
import {
  applySnapshot, collectSnapshotTree, restoreSnapshot, snapshotPath,
  type AblateOptions, type AblateResult, type Mutation, type RestoreOptions, type RestoreResult,
} from "./snapshot.js";

type Framework = "trellis" | "cursor" | "claude" | "copilot" | "windsurf" | "spec-kit" | "bmad" | "project";
type Category = "conflict" | "rule-source" | "memory-source" | "gate-source";
export interface MigrationItem { framework: Framework; category: Category; path: string; globs?: string[] }

const trellisStart = "<!-- TRELLIS:START -->";
const trellisEnd = "<!-- TRELLIS:END -->";

function isObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function referencesTrellis(value: unknown, hooks: string[]): boolean {
  if (Array.isArray(value)) return value.some((item) => referencesTrellis(item, hooks));
  if (isObject(value)) return Object.values(value).some((item) => referencesTrellis(item, hooks));
  if (typeof value !== "string") return false;
  if (/trellis/i.test(value)) return true;
  const command = value.replace(/\\/g, "/");
  return hooks.some((hook) => {
    const offset = command.indexOf(hook);
    if (offset < 0) return false;
    const next = command[offset + hook.length];
    return next === undefined || /[\s"';&|)]/.test(next);
  });
}

function regular(root: string, relative: string, directory = false): boolean {
  const parts = relative.split("/");
  let parent = root;
  for (const part of parts.slice(0, -1)) {
    parent = path.join(parent, part);
    if (!fs.lstatSync(parent, { throwIfNoEntry: false })?.isDirectory()) return false;
  }
  const stat = fs.lstatSync(path.join(root, relative), { throwIfNoEntry: false });
  return !!stat && (directory ? stat.isDirectory() : stat.isFile());
}

function scanFiles(root: string, relative: string, recursive = false): string[] {
  if (!regular(root, relative, true)) return [];
  const files: string[] = [];
  for (const item of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
    const file = `${relative}/${item.name}`;
    if (item.isFile()) files.push(file);
    else if (recursive && item.isDirectory()) files.push(...scanFiles(root, file, true));
  }
  return files;
}

function cursorGlobs(text: string): string[] {
  const globs = parseFrontmatter(text).data.globs;
  const list = typeof globs === "string" ? globs.split(/,(?![^{}]*\})/) : Array.isArray(globs) ? globs : [];
  return list.filter((glob): glob is string => typeof glob === "string" && !!glob.trim()).map((glob) => toPosixPath(glob.trim()));
}

export function scanMigration(root: string): { items: MigrationItem[] } {
  const items: MigrationItem[] = [];
  const add = (framework: Framework, category: Category, file: string, globs?: string[]) => {
    items.push({ framework, category, path: file, ...(globs?.length ? { globs } : {}) });
  };
  const read = (file: string) => fs.readFileSync(path.join(root, file), "utf8");
  if (regular(root, ".trellis", true)) add("trellis", "conflict", ".trellis");
  if (regular(root, "AGENTS.md") && read("AGENTS.md").includes(trellisStart)) add("trellis", "conflict", "AGENTS.md");
  for (const dir of [".claude/agents", ".cursor/agents", ".cursor/commands"]) {
    if (!regular(root, dir, true)) continue;
    for (const entry of fs.readdirSync(path.join(root, dir), { withFileTypes: true })) {
      const pattern = dir.endsWith("commands") ? /^trellis/i : /^trellis-/i;
      if ((entry.isFile() || entry.isDirectory()) && pattern.test(entry.name)) add("trellis", "conflict", `${dir}/${entry.name}`);
    }
  }
  const hooks = scanFiles(root, ".claude/hooks", true).filter((file) => /trellis/i.test(file) || /trellis/i.test(read(file)));
  for (const hook of hooks) add("trellis", "conflict", hook);
  if (regular(root, ".claude/settings.json")) {
    const raw = read(".claude/settings.json");
    try {
      const settings: unknown = JSON.parse(raw);
      if (isObject(settings) && referencesTrellis(settings.hooks, hooks)) add("trellis", "conflict", ".claude/settings.json");
      else if (referencesTrellis(settings, hooks)) add("trellis", "rule-source", ".claude/settings.json");
    } catch {
      if (referencesTrellis(raw, hooks)) add("trellis", "conflict", ".claude/settings.json");
    }
  }
  if (regular(root, ".cursorrules")) add("cursor", "conflict", ".cursorrules");
  for (const file of scanFiles(root, ".cursor/rules").filter((file) => file.endsWith(".mdc"))) add("cursor", "rule-source", file, cursorGlobs(read(file)));
  for (const [framework, file] of [["claude", "CLAUDE.md"], ["copilot", ".github/copilot-instructions.md"], ["windsurf", ".windsurfrules"]] as const) {
    if (regular(root, file)) add(framework, "rule-source", file);
  }
  for (const [framework, directory] of [["trellis", ".trellis/spec"], ["windsurf", ".windsurf/rules"], ["spec-kit", ".specify"], ["bmad", "_bmad"]] as const) {
    if (!regular(root, directory, true)) continue;
    add(framework, "rule-source", directory);
    for (const file of scanFiles(root, directory, true).filter((file) => /\.mdc?$/i.test(file) || framework === "windsurf")) add(framework, "rule-source", file);
  }
  for (const [framework, directory] of [["project", "docs/adr"], ["trellis", ".trellis/tasks/archive"]] as const) {
    if (regular(root, directory, true)) add(framework, "memory-source", directory);
  }
  for (const file of ["package.json", "Makefile", ...scanFiles(root, ".github/workflows").filter((name) => /\.ya?ml$/.test(name))]) {
    if (regular(root, file)) add("project", "gate-source", file);
  }
  items.sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
  return { items };
}

export function planMigration(root: string) {
  const { items } = scanMigration(root);
  return {
    conflicts: items.filter((item) => item.category === "conflict"),
    suggestedGates: detectProjectGates(root),
    suggestedMemorySources: items.filter((item) => item.category === "memory-source").map((item) => item.path),
    ruleSources: items.filter((item) => (item.category === "rule-source" || item.path === ".cursorrules") && regular(root, item.path)),
  };
}

function stripTrellisBlocks(original: Buffer): Buffer | null {
  const source = new TextDecoder("utf8", { fatal: true, ignoreBOM: true }).decode(original);
  let rest = source;
  let result = "";
  while (rest.includes(trellisStart)) {
    const start = rest.indexOf(trellisStart);
    const end = rest.indexOf(trellisEnd);
    const nested = rest.indexOf(trellisStart, start + trellisStart.length);
    if (end < start || (nested !== -1 && nested < end)) throw new Error("Malformed Trellis markers in AGENTS.md; no files were changed.");
    const block = rest.slice(start, end);
    if (block.includes("<!-- KEEL:")) throw new Error("Overlapping Keel and Trellis markers in AGENTS.md; no files were changed.");
    result += rest.slice(0, start);
    rest = rest.slice(end + trellisEnd.length).replace(/^\r?\n/, "");
  }
  if (rest.includes(trellisEnd)) throw new Error("Unpaired Trellis end marker in AGENTS.md.");
  result += rest;
  return result.length ? Buffer.from(result) : null;
}

function cleanSettings(original: Buffer, retiredHooks: string[]): Buffer | null {
  const settings: unknown = JSON.parse(original.toString("utf8"));
  if (!isObject(settings) || !isObject(settings.hooks)) throw new Error("Cannot safely isolate Trellis hooks in .claude/settings.json; review it manually.");
  let changed = false;
  for (const [event, groups] of Object.entries(settings.hooks)) {
    if (!Array.isArray(groups)) continue;
    const kept = groups.flatMap((group) => {
      if (!isObject(group) || !Array.isArray(group.hooks)) {
        if (referencesTrellis(group, retiredHooks)) { changed = true; return []; }
        return [group];
      }
      const hooks = group.hooks.filter((hook) => !referencesTrellis(hook, retiredHooks));
      if (hooks.length === group.hooks.length) return [group];
      changed = true;
      return hooks.length ? [{ ...group, hooks }] : [];
    });
    if (kept.length) settings.hooks[event] = kept;
    else if (groups.length) delete settings.hooks[event];
  }
  if (!changed) throw new Error("Cannot safely isolate Trellis hooks in .claude/settings.json; review it manually.");
  if (!Object.keys(settings.hooks).length) delete settings.hooks;
  return Object.keys(settings).length ? Buffer.from(`${JSON.stringify(settings, null, 2)}\n`) : null;
}

export function cleanupMigration(root: string, options: AblateOptions = {}): AblateResult {
  const changes: Mutation[] = [];
  const directories: string[] = [];
  const conflicts = scanMigration(root).items.filter((item) => item.category === "conflict");
  const hooks = conflicts.map((item) => item.path).filter((file) => file.startsWith(".claude/hooks/"));
  for (const item of conflicts) {
    if (regular(root, item.path, true)) {
      collectSnapshotTree(root, item.path, changes, directories);
    } else if (item.path === "AGENTS.md" || item.path === ".claude/settings.json") {
      const original = fs.readFileSync(snapshotPath(root, item.path));
      const replacement = item.path === "AGENTS.md" ? stripTrellisBlocks(original) : cleanSettings(original, hooks);
      changes.push({ file: item.path, original, replacement });
    } else {
      changes.push({ file: item.path, replacement: null });
    }
  }
  return applySnapshot(root, "migrate", changes, options, directories);
}

export function rollbackMigration(root: string, options: RestoreOptions = {}): RestoreResult {
  return restoreSnapshot(root, "migrate", options);
}
