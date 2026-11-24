import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { initRepo } from "../src/core/init.js";
import { handleMcpRequest } from "../src/core/mcp.js";
import { addRule } from "../src/core/rules.js";

const tempDirs: string[] = [];

function makeTempRepo(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "keel-mcp-"));
  tempDirs.push(dir);
  initRepo(dir);
  return dir;
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("MCP server protocol", () => {
  it("handles initialize, tools/list, and tools/call end-to-end", () => {
    const root = makeTempRepo();
    addRule(root, "db-tx", {
      summary: "Always wrap balance updates in a DB transaction",
      severity: "invariant",
      paths: ["src/billing/**"],
    });

    const initRes = handleMcpRequest(root, { id: 1, method: "initialize" });
    expect(initRes?.result).toMatchObject({
      serverInfo: { name: "keel", version: "1.1.0" },
    });

    const listRes = handleMcpRequest(root, { id: 2, method: "tools/list" });
    const tools = (listRes?.result as { tools: Array<{ name: string }> }).tools;
    expect(tools.map((t) => t.name)).toContain("keel_rules_match");
    expect(tools.map((t) => t.name)).toContain("keel_mem_search");
    expect(tools.map((t) => t.name)).toContain("keel_verify");

    const matchRes = handleMcpRequest(root, {
      id: 3,
      method: "tools/call",
      params: {
        name: "keel_rules_match",
        arguments: { paths: ["src/billing/charge.ts"] },
      },
    });
    const matchText = (
      matchRes?.result as { content: Array<{ text: string }> }
    ).content[0]!.text;
    expect(matchText).toContain("db-tx");

    const createTaskRes = handleMcpRequest(root, {
      id: 4,
      method: "tools/call",
      params: {
        name: "keel_task_create",
        arguments: { title: "Migrate ledger schema", slug: "ledger-schema" },
      },
    });
    const createdText = (
      createTaskRes?.result as { content: Array<{ text: string }> }
    ).content[0]!.text;
    const created = JSON.parse(createdText) as { id: string };
    expect(created.id).toContain("ledger-schema");

    const unknownMethod = handleMcpRequest(root, {
      id: 5,
      method: "unknown/method",
    });
    expect(unknownMethod?.error?.code).toBe(-32601);
  });
});
