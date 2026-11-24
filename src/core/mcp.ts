import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { KEEL_VERSION } from "../version.js";
import { getGitChangedFiles } from "./git.js";
import { listMemorySessions, searchMemory } from "./mem.js";
import { listRules, matchRules, validateRules } from "./rules.js";
import { createTask, getTask, listTasks } from "./tasks.js";
import { runVerify } from "./verify.js";

export interface JsonRpcRequest {
  jsonrpc?: string;
  id?: string | number | null;
  method: string;
  params?: Record<string, unknown>;
}

export interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: string | number | null;
  result?: unknown;
  error?: {
    code: number;
    message: string;
  };
}

export interface McpToolDefinition {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export const MCP_PROTOCOL_VERSION = "2024-11-05";

export const KEEL_MCP_TOOLS: McpToolDefinition[] = [
  {
    name: "keel_rules_list",
    description: "List all engineering rules, invariants, and ADRs in .keel/rules/.",
    inputSchema: {
      type: "object",
      properties: {},
      additionalProperties: false,
    },
  },
  {
    name: "keel_rules_match",
    description:
      "Match scoped engineering invariants for target repository paths or current git changes.",
    inputSchema: {
      type: "object",
      properties: {
        paths: {
          type: "array",
          items: { type: "string" },
          description: "Repository-relative file paths to match.",
        },
        git: {
          type: "boolean",
          description: "If true, include current git changed files.",
        },
        includeGlobal: {
          type: "boolean",
          description: "Include global rules with no path filter (default true).",
        },
      },
      additionalProperties: false,
    },
  },
  {
    name: "keel_rules_check",
    description: "Validate frontmatter, IDs, and severities of all .keel/rules/ files.",
    inputSchema: {
      type: "object",
      properties: { stale: { type: "boolean", description: "Check unmatched paths and rule hygiene." } },
      additionalProperties: false,
    },
  },
  {
    name: "keel_task_list",
    description: "List lightweight task specs under .keel/tasks/.",
    inputSchema: {
      type: "object",
      properties: {
        status: {
          type: "string",
          enum: ["open", "done"],
        },
        includeArchived: {
          type: "boolean",
        },
      },
      additionalProperties: false,
    },
  },
  {
    name: "keel_task_create",
    description: "Create a lightweight task directory with spec.md and meta.json.",
    inputSchema: {
      type: "object",
      properties: {
        title: { type: "string" },
        slug: { type: "string" },
      },
      required: ["title"],
      additionalProperties: false,
    },
  },
  {
    name: "keel_task_show",
    description: "Read a task's metadata and living spec.md contents.",
    inputSchema: {
      type: "object",
      properties: {
        id: { type: "string" },
      },
      required: ["id"],
      additionalProperties: false,
    },
  },
  {
    name: "keel_verify",
    description: "Run configured deterministic verification gates from .keel/config.yaml.",
    inputSchema: {
      type: "object",
      properties: {
        gates: {
          type: "array",
          items: { type: "string" },
          description: "Optional subset of gate names to run.",
        },
        failFast: {
          type: "boolean",
        },
        git: { type: "boolean", description: "Filter by uncommitted changes unless gate names are supplied." },
      },
      additionalProperties: false,
    },
  },
  {
    name: "keel_mem_search",
    description:
      "Search across local AI coding transcripts and repository Keel task/rule artifacts.",
    inputSchema: {
      type: "object",
      properties: {
        query: { type: "string" },
        source: { type: "string" },
        limit: { type: "number" },
      },
      required: ["query"],
      additionalProperties: false,
    },
  },
  {
    name: "keel_mem_sessions",
    description: "List recent local AI coding sessions and repository artifacts.",
    inputSchema: {
      type: "object",
      properties: {
        source: { type: "string" },
        limit: { type: "number" },
      },
      additionalProperties: false,
    },
  },
];

function toolTextResult(payload: unknown, isError = false): {
  content: Array<{ type: "text"; text: string }>;
  isError?: boolean;
} {
  return {
    content: [
      {
        type: "text",
        text:
          typeof payload === "string"
            ? payload
            : JSON.stringify(payload, null, 2),
      },
    ],
    ...(isError ? { isError: true } : {}),
  };
}

function dispatchToolCall(
  repoRoot: string,
  name: string,
  args: Record<string, unknown>,
): { content: Array<{ type: "text"; text: string }>; isError?: boolean } {
  try {
    switch (name) {
      case "keel_rules_list":
        return toolTextResult(listRules(repoRoot));

      case "keel_rules_match": {
        const inputPaths = Array.isArray(args.paths)
          ? args.paths.filter((p): p is string => typeof p === "string")
          : [];
        const gitPaths = args.git === true ? getGitChangedFiles(repoRoot) : [];
        const targets = [...new Set([...inputPaths, ...gitPaths])];
        const includeGlobal = args.includeGlobal !== false;
        return toolTextResult(matchRules(repoRoot, targets, { includeGlobal }));
      }

      case "keel_rules_check": {
        const report = validateRules(repoRoot, { stale: args.stale === true });
        return toolTextResult(report, !report.valid);
      }

      case "keel_task_list": {
        const status =
          args.status === "open" || args.status === "done"
            ? args.status
            : undefined;
        return toolTextResult(
          listTasks(repoRoot, {
            status,
            includeArchived: args.includeArchived === true,
          }),
        );
      }

      case "keel_task_create": {
        if (typeof args.title !== "string" || !args.title.trim()) {
          return toolTextResult("Missing required string argument: title", true);
        }
        const slug = typeof args.slug === "string" ? args.slug : undefined;
        return toolTextResult(createTask(repoRoot, args.title, { slug }));
      }

      case "keel_task_show": {
        if (typeof args.id !== "string" || !args.id.trim()) {
          return toolTextResult("Missing required string argument: id", true);
        }
        const record = getTask(repoRoot, args.id);
        const spec = fs.readFileSync(path.join(repoRoot, record.specFile), "utf-8");
        return toolTextResult({ ...record, spec });
      }

      case "keel_verify": {
        const gates = Array.isArray(args.gates)
          ? args.gates.filter((g): g is string => typeof g === "string")
          : undefined;
        const report = runVerify(repoRoot, {
          gateNames: gates,
          git: args.git === true,
          failFast: args.failFast === true,
        });
        return toolTextResult(report, !report.passed);
      }

      case "keel_mem_search": {
        if (typeof args.query !== "string" || !args.query.trim()) {
          return toolTextResult("Missing required string argument: query", true);
        }
        return toolTextResult(
          searchMemory(repoRoot, args.query, {
            source: typeof args.source === "string" ? args.source : undefined,
            limit: typeof args.limit === "number" ? args.limit : undefined,
          }),
        );
      }

      case "keel_mem_sessions":
        return toolTextResult(
          listMemorySessions(repoRoot, {
            source: typeof args.source === "string" ? args.source : undefined,
            limit: typeof args.limit === "number" ? args.limit : undefined,
          }),
        );

      default:
        return toolTextResult(`Unknown tool: ${name}`, true);
    }
  } catch (err) {
    return toolTextResult(
      err instanceof Error ? err.message : String(err),
      true,
    );
  }
}

export function handleMcpRequest(
  repoRoot: string,
  req: JsonRpcRequest,
): JsonRpcResponse | null {
  const id = req.id ?? null;

  // Notifications carry no id and expect no response.
  if (req.id === undefined && req.method.startsWith("notifications/")) {
    return null;
  }

  switch (req.method) {
    case "initialize":
      return {
        jsonrpc: "2.0",
        id,
        result: {
          protocolVersion: MCP_PROTOCOL_VERSION,
          capabilities: {
            tools: {},
          },
          serverInfo: {
            name: "keel",
            version: KEEL_VERSION,
          },
        },
      };

    case "ping":
      return {
        jsonrpc: "2.0",
        id,
        result: {},
      };

    case "tools/list":
      return {
        jsonrpc: "2.0",
        id,
        result: {
          tools: KEEL_MCP_TOOLS,
        },
      };

    case "tools/call": {
      const params = req.params ?? {};
      const name = typeof params.name === "string" ? params.name : "";
      const args =
        params.arguments && typeof params.arguments === "object"
          ? (params.arguments as Record<string, unknown>)
          : {};
      if (!name) {
        return {
          jsonrpc: "2.0",
          id,
          error: { code: -32602, message: "Missing tool name" },
        };
      }
      return {
        jsonrpc: "2.0",
        id,
        result: dispatchToolCall(repoRoot, name, args),
      };
    }

    default:
      return {
        jsonrpc: "2.0",
        id,
        error: {
          code: -32601,
          message: `Method not found: ${req.method}`,
        },
      };
  }
}

export async function startMcpStdioServer(repoRoot: string): Promise<void> {
  const rl = readline.createInterface({
    input: process.stdin,
    crlfDelay: Infinity,
  });

  for await (const line of rl) {
    const trimmed = line.trim();
    if (!trimmed) continue;

    let req: JsonRpcRequest;
    try {
      req = JSON.parse(trimmed) as JsonRpcRequest;
    } catch {
      const parseErr: JsonRpcResponse = {
        jsonrpc: "2.0",
        id: null,
        error: { code: -32700, message: "Parse error" },
      };
      process.stdout.write(`${JSON.stringify(parseErr)}\n`);
      continue;
    }

    const res = handleMcpRequest(repoRoot, req);
    if (res) {
      process.stdout.write(`${JSON.stringify(res)}\n`);
    }
  }
}
