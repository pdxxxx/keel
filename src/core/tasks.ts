import fs from "node:fs";
import path from "node:path";
import { ARCHIVE_DIR, keelPath, TASKS_DIR, toPosixPath } from "./paths.js";
import type { TaskMeta, TaskRecord, TaskStatus } from "./types.js";

function slugify(input: string): string {
  return input
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
}

function formatDatePrefix(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function taskDirectory(repoRoot: string, id: string): string {
  const parts = id.split(/[\\/]/).filter((part) => part !== "" && part !== ".");
  if (
    /^[\\/]/.test(id) || id.includes("\0") || parts.length === 0 ||
    parts.some((part) => part === ".." || /^[a-z]:/i.test(part))
  ) {
    throw new Error(`Task not found: ${id}`);
  }
  return keelPath(repoRoot, TASKS_DIR, parts.join("/"));
}

function readTaskMeta(taskDir: string): TaskMeta | null {
  const metaPath = path.join(taskDir, "meta.json");
  if (!fs.existsSync(metaPath)) return null;

  try {
    const raw = JSON.parse(fs.readFileSync(metaPath, "utf-8")) as Partial<TaskMeta>;
    if (
      typeof raw.id !== "string" ||
      typeof raw.title !== "string" ||
      (raw.status !== "open" && raw.status !== "done")
    ) {
      return null;
    }
    return {
      id: raw.id,
      title: raw.title,
      status: raw.status,
      createdAt: typeof raw.createdAt === "string" ? raw.createdAt : "",
      updatedAt: typeof raw.updatedAt === "string" ? raw.updatedAt : "",
    };
  } catch {
    return null;
  }
}

export interface CreateTaskOptions {
  slug?: string;
  now?: Date;
}

export function createTask(
  repoRoot: string,
  title: string,
  options: CreateTaskOptions = {},
): TaskRecord {
  const trimmedTitle = title.trim();
  if (!trimmedTitle) {
    throw new Error("Task title cannot be empty.");
  }

  const now = options.now ?? new Date();
  const slug = slugify(options.slug?.trim() || trimmedTitle) || "task";
  const baseId = `${formatDatePrefix(now)}-${slug}`;
  const tasksRoot = keelPath(repoRoot, TASKS_DIR);
  fs.mkdirSync(tasksRoot, { recursive: true });

  let id = baseId;
  let counter = 2;
  while (fs.existsSync(path.join(tasksRoot, id))) {
    id = `${baseId}-${counter}`;
    counter += 1;
  }

  const taskDir = path.join(tasksRoot, id);
  fs.mkdirSync(taskDir, { recursive: true });

  const timestamp = now.toISOString();
  const meta: TaskMeta = {
    id,
    title: trimmedTitle,
    status: "open",
    createdAt: timestamp,
    updatedAt: timestamp,
  };

  const specContent = `# ${trimmedTitle}

## Goal

- What problem are we solving?

## Non-Goals

- What is explicitly out of scope?

## Acceptance Criteria

- [ ] Concrete, verifiable outcome

## Decisions & Notes

- Record tradeoffs or links to \`research/\` notes here so they survive session compaction.
`;

  fs.writeFileSync(
    path.join(taskDir, "meta.json"),
    `${JSON.stringify(meta, null, 2)}\n`,
    "utf-8",
  );
  fs.writeFileSync(path.join(taskDir, "spec.md"), specContent, "utf-8");

  return {
    ...meta,
    dir: toPosixPath(path.relative(repoRoot, taskDir)),
    specFile: toPosixPath(path.relative(repoRoot, path.join(taskDir, "spec.md"))),
  };
}

export function listTasks(
  repoRoot: string,
  options: { status?: TaskStatus; includeArchived?: boolean } = {},
): TaskRecord[] {
  const tasksRoot = keelPath(repoRoot, TASKS_DIR);
  if (!fs.existsSync(tasksRoot)) return [];

  const dirsToScan: string[] = [tasksRoot];
  if (options.includeArchived) {
    const archiveRoot = path.join(tasksRoot, ARCHIVE_DIR);
    if (fs.existsSync(archiveRoot)) {
      dirsToScan.push(archiveRoot);
    }
  }

  const records: TaskRecord[] = [];

  for (const baseDir of dirsToScan) {
    for (const entry of fs.readdirSync(baseDir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      if (baseDir === tasksRoot && entry.name === ARCHIVE_DIR) continue;

      const fullDir = path.join(baseDir, entry.name);
      const meta = readTaskMeta(fullDir);
      if (!meta) continue;
      if (options.status && meta.status !== options.status) continue;

      records.push({
        ...meta,
        dir: toPosixPath(path.relative(repoRoot, fullDir)),
        specFile: toPosixPath(
          path.relative(repoRoot, path.join(fullDir, "spec.md")),
        ),
      });
    }
  }

  return records.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
}

export function getTask(repoRoot: string, id: string): TaskRecord {
  const taskDir = taskDirectory(repoRoot, id);
  const meta = readTaskMeta(taskDir);
  if (!meta) {
    throw new Error(`Task not found: ${id}`);
  }
  return {
    ...meta,
    dir: toPosixPath(path.relative(repoRoot, taskDir)),
    specFile: toPosixPath(path.relative(repoRoot, path.join(taskDir, "spec.md"))),
  };
}

export function setTaskStatus(
  repoRoot: string,
  id: string,
  status: TaskStatus,
  now: Date = new Date(),
): TaskRecord {
  const taskDir = taskDirectory(repoRoot, id);
  const meta = readTaskMeta(taskDir);
  if (!meta) {
    throw new Error(`Task not found: ${id}`);
  }

  const updated: TaskMeta = {
    ...meta,
    status,
    updatedAt: now.toISOString(),
  };

  fs.writeFileSync(
    path.join(taskDir, "meta.json"),
    `${JSON.stringify(updated, null, 2)}\n`,
    "utf-8",
  );

  return {
    ...updated,
    dir: toPosixPath(path.relative(repoRoot, taskDir)),
    specFile: toPosixPath(path.relative(repoRoot, path.join(taskDir, "spec.md"))),
  };
}

export function archiveTask(
  repoRoot: string,
  id: string,
  now: Date = new Date(),
): TaskRecord {
  const updated = setTaskStatus(repoRoot, id, "done", now);
  const tasksRoot = keelPath(repoRoot, TASKS_DIR);
  const sourceDir = path.join(repoRoot, updated.dir);
  const archiveRoot = path.join(tasksRoot, ARCHIVE_DIR);
  fs.mkdirSync(archiveRoot, { recursive: true });

  const destDir = path.join(archiveRoot, path.relative(tasksRoot, sourceDir));
  if (fs.existsSync(destDir)) {
    throw new Error(`Archived task already exists: ${id}`);
  }

  fs.renameSync(sourceDir, destDir);

  return {
    ...updated,
    dir: toPosixPath(path.relative(repoRoot, destDir)),
    specFile: toPosixPath(path.relative(repoRoot, path.join(destDir, "spec.md"))),
  };
}
