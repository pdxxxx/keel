import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  getMemorySession,
  listMemorySessions,
  listMemorySources,
  searchMemory,
} from "../src/core/mem.js";
import { createTask } from "../src/core/tasks.js";

const tempDirs: string[] = [];

function makeTempDir(prefix: string): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("cross-tool memory engine", () => {
  it("indexes repo artifacts, Claude JSONL, and Codex JSONL transcripts", () => {
    const repoRoot = makeTempDir("keel-mem-repo-");
    const fakeHome = makeTempDir("keel-mem-home-");

    // 1. Keel repo task spec
    const task = createTask(repoRoot, "Timeout calibration", {
      slug: "timeout-calibration",
    });
    fs.appendFileSync(
      path.join(repoRoot, task.specFile),
      "\nWe raised the webhook timeout to 15s because payment retries spike at midnight.\n",
      "utf-8",
    );

    // 2. Simulated Claude Code JSONL transcript
    const claudeDir = path.join(fakeHome, ".claude", "projects", "demo");
    fs.mkdirSync(claudeDir, { recursive: true });
    fs.writeFileSync(
      path.join(claudeDir, "session-001.jsonl"),
      [
        JSON.stringify({
          timestamp: "2026-11-20T10:00:00Z",
          message: {
            role: "user",
            content: "Why did payment webhook retries fail at midnight?",
          },
        }),
        JSON.stringify({
          timestamp: "2026-11-20T10:01:00Z",
          message: {
            role: "assistant",
            content: [
              {
                type: "text",
                text: "Because the gateway latency hits 12s during settlement batching.",
              },
            ],
          },
        }),
      ].join("\n"),
      "utf-8",
    );

    // 3. Simulated Codex JSONL transcript
    const codexDir = path.join(fakeHome, ".codex", "sessions", "2026");
    fs.mkdirSync(codexDir, { recursive: true });
    fs.writeFileSync(
      path.join(codexDir, "rollout-01.jsonl"),
      JSON.stringify({
        timestamp: "2026-11-21T08:00:00Z",
        payload: {
          role: "user",
          content: "Keep payment webhook idempotency keys for 24 hours.",
        },
      }),
      "utf-8",
    );

    const sources = listMemorySources(repoRoot, { homeDir: fakeHome });
    expect(sources.find((s) => s.source === "keel")?.sessionCount).toBe(1);
    expect(sources.find((s) => s.source === "claude")?.sessionCount).toBe(1);
    expect(sources.find((s) => s.source === "codex")?.sessionCount).toBe(1);

    const sessions = listMemorySessions(repoRoot, { homeDir: fakeHome });
    expect(sessions.length).toBe(3);

    const hits = searchMemory(repoRoot, "payment webhook settlement", {
      homeDir: fakeHome,
    });
    expect(hits.length).toBeGreaterThanOrEqual(2);
    expect(hits.some((h) => h.source === "claude")).toBe(true);
    expect(hits.some((h) => h.source === "keel")).toBe(true);

    const claudeSession = sessions.find((s) => s.source === "claude")!;
    const detail = getMemorySession(repoRoot, claudeSession.id, {
      homeDir: fakeHome,
    });
    expect(detail.turns).toHaveLength(2);
    expect(detail.turns[1]?.text).toContain("settlement batching");
  });
});
