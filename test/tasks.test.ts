import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  archiveTask,
  createTask,
  getTask,
  listTasks,
  setTaskStatus,
} from "../src/core/tasks.js";

const tempDirs: string[] = [];

function makeTempRepo(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "keel-tasks-"));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("task lifecycle", () => {
  it("creates, lists, updates, and archives lightweight tasks", () => {
    const root = makeTempRepo();
    const fixedDate = new Date("2026-11-24T12:00:00.000Z");

    const created = createTask(root, "Refactor session refresh", {
      slug: "session-refresh",
      now: fixedDate,
    });
    expect(created.id).toBe("2026-11-24-session-refresh");
    expect(created.status).toBe("open");
    expect(fs.existsSync(path.join(root, created.specFile))).toBe(true);

    const fetched = getTask(root, created.id);
    expect(fetched.title).toBe("Refactor session refresh");

    const done = setTaskStatus(root, created.id, "done", fixedDate);
    expect(done.status).toBe("done");

    const archived = archiveTask(root, created.id, fixedDate);
    expect(archived.dir).toBe(".keel/tasks/archive/2026-11-24-session-refresh");

    expect(listTasks(root)).toHaveLength(0);
    expect(listTasks(root, { includeArchived: true })).toHaveLength(1);
    expect(getTask(root, `archive/${created.id}`)).toEqual(archived);
    expect(getTask(root, `.\\archive\\${created.id}`)).toEqual(archived);
  });

  it("rejects parent segments and rooted IDs before reading, updating or archiving a task", () => {
    const root = makeTempRepo();
    const created = createTask(root, "Protected task");
    const metaFile = path.join(root, created.dir, "meta.json");
    const original = fs.readFileSync(metaFile, "utf8");
    const invalid = [
      "..", `archive/../${created.id}`, `archive\\..\\${created.id}`,
      `/${created.id}`, `\\${created.id}`, path.resolve(root, created.dir),
      `C:\\${created.id}`, `C:${created.id}`, `\\\\host\\share\\${created.id}`,
    ];
    for (const id of invalid) {
      const expected = new Error(`Task not found: ${id}`);
      expect(() => getTask(root, id)).toThrow(expected);
      expect(() => setTaskStatus(root, id, "done")).toThrow(expected);
      expect(() => archiveTask(root, id)).toThrow(expected);
      expect(fs.readFileSync(metaFile, "utf8")).toBe(original);
    }
    expect(fs.existsSync(path.join(root, ".keel", "tasks", "archive"))).toBe(false);
  });
});
