import fs from "node:fs";
import path from "node:path";
import YAML from "yaml";
import { matchGlob } from "./glob.js";
import { keelPath, RULES_DIR, toPosixPath } from "./paths.js";
import type {
  RuleEntry,
  RuleIssue,
  RuleSeverity,
  RuleValidationReport,
} from "./types.js";

const VALID_SEVERITIES = new Set<RuleSeverity>([
  "invariant",
  "guideline",
  "adr",
]);

function walkMarkdownFiles(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  const results: string[] = [];

  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...walkMarkdownFiles(full));
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      results.push(full);
    }
  }

  return results.sort();
}

function parseFrontmatter(content: string): {
  hasFrontmatter: boolean;
  data: Record<string, unknown>;
  body: string;
  parseError?: string;
} {
  const opening = /^---\r?\n/.exec(content);
  const closing = opening && /\r?\n---(?:\r?\n|$)/.exec(content);
  if (!opening || !closing) {
    return { hasFrontmatter: false, data: {}, body: content };
  }

  const header = content.slice(opening[0].length, closing.index);
  const body = content.slice(closing.index + closing[0].length);
  try {
    const parsed = YAML.parse(header);
    const data =
      parsed && typeof parsed === "object"
        ? (parsed as Record<string, unknown>)
        : {};
    return { hasFrontmatter: true, data, body };
  } catch (err) {
    return {
      hasFrontmatter: true,
      data: {},
      body,
      parseError: err instanceof Error ? err.message : String(err),
    };
  }
}

function extractFallbackSummary(body: string): string {
  for (const line of body.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    return trimmed.replace(/^#+\s*/, "");
  }
  return "";
}

export function listRules(repoRoot: string): RuleEntry[] {
  const rulesRoot = keelPath(repoRoot, RULES_DIR);
  const files = walkMarkdownFiles(rulesRoot);
  const entries: RuleEntry[] = [];

  for (const fullPath of files) {
    const relFromRules = toPosixPath(path.relative(rulesRoot, fullPath));
    const content = fs.readFileSync(fullPath, "utf-8");
    const { data, body } = parseFrontmatter(content);

    if (relFromRules === "index.md" && typeof data.id !== "string") {
      continue;
    }

    const defaultId = relFromRules.replace(/\.md$/, "").replace(/\//g, ":");
    const id =
      typeof data.id === "string" && data.id.trim().length > 0
        ? data.id.trim()
        : defaultId;

    const summary =
      typeof data.summary === "string" && data.summary.trim().length > 0
        ? data.summary.trim()
        : extractFallbackSummary(body);

    const rawSeverity =
      typeof data.severity === "string" ? data.severity.trim() : "guideline";
    const severity: RuleSeverity = VALID_SEVERITIES.has(
      rawSeverity as RuleSeverity,
    )
      ? (rawSeverity as RuleSeverity)
      : "guideline";

    const paths: string[] = Array.isArray(data.paths)
      ? data.paths
          .filter(
            (p): p is string => typeof p === "string" && p.trim().length > 0,
          )
          .map((p) => toPosixPath(p.trim()))
      : [];

    entries.push({
      id,
      file: toPosixPath(path.relative(repoRoot, fullPath)),
      summary,
      severity,
      paths,
      global: paths.length === 0,
    });
  }

  return entries;
}

export function matchRules(
  repoRoot: string,
  targetPaths: string[],
  options: { includeGlobal?: boolean } = {},
): RuleEntry[] {
  const includeGlobal = options.includeGlobal ?? true;
  const normalizedTargets = targetPaths.map((p) => toPosixPath(p));
  const allRules = listRules(repoRoot);

  return allRules.filter((rule) => {
    if (rule.global) return includeGlobal;
    return normalizedTargets.some((target) =>
      rule.paths.some((pattern) => matchGlob(pattern, target)),
    );
  });
}

export interface AddRuleOptions {
  summary: string;
  severity?: RuleSeverity;
  paths?: string[];
  body?: string;
  force?: boolean;
}

export function addRule(
  repoRoot: string,
  id: string,
  options: AddRuleOptions,
): RuleEntry {
  const cleanId = id
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  if (!cleanId) {
    throw new Error("Rule id must contain alphanumeric characters.");
  }

  const severity: RuleSeverity =
    options.severity && VALID_SEVERITIES.has(options.severity)
      ? options.severity
      : "invariant";
  const summary = options.summary.trim();
  if (!summary) {
    throw new Error("Rule summary cannot be empty.");
  }

  const paths = (options.paths ?? [])
    .map((p) => toPosixPath(p.trim()))
    .filter((p) => p.length > 0);

  const rulesRoot = keelPath(repoRoot, RULES_DIR);
  fs.mkdirSync(rulesRoot, { recursive: true });

  const filePath = path.join(rulesRoot, `${cleanId}.md`);
  if (fs.existsSync(filePath) && !options.force) {
    throw new Error(`Rule file already exists: .keel/rules/${cleanId}.md`);
  }

  const frontmatterObj: Record<string, unknown> = {
    id: cleanId,
    summary,
    severity,
  };
  if (paths.length > 0) {
    frontmatterObj.paths = paths;
  }

  const frontmatterYaml = YAML.stringify(frontmatterObj).trimEnd();
  const body =
    options.body?.trim() ||
    `# ${cleanId}\n\n## Why\n\nExplain the non-obvious invariant, tradeoff, or failure mode this prevents.\n`;

  fs.writeFileSync(
    filePath,
    `---\n${frontmatterYaml}\n---\n\n${body}\n`,
    "utf-8",
  );

  return {
    id: cleanId,
    file: toPosixPath(path.relative(repoRoot, filePath)),
    summary,
    severity,
    paths,
    global: paths.length === 0,
  };
}

export function validateRules(repoRoot: string): RuleValidationReport {
  const rulesRoot = keelPath(repoRoot, RULES_DIR);
  const files = walkMarkdownFiles(rulesRoot);
  const issues: RuleIssue[] = [];
  const seenIds = new Map<string, string>();
  let checked = 0;

  for (const fullPath of files) {
    const relFromRules = toPosixPath(path.relative(rulesRoot, fullPath));
    const relFromRepo = toPosixPath(path.relative(repoRoot, fullPath));
    const content = fs.readFileSync(fullPath, "utf-8");
    const parsed = parseFrontmatter(content);

    if (relFromRules === "index.md" && !parsed.hasFrontmatter) {
      continue;
    }

    checked += 1;

    if (parsed.parseError) {
      issues.push({
        file: relFromRepo,
        message: `Invalid YAML frontmatter: ${parsed.parseError}`,
      });
      continue;
    }

    if (!parsed.hasFrontmatter) {
      issues.push({
        file: relFromRepo,
        message: "Missing YAML frontmatter (expected id, summary, severity).",
      });
      continue;
    }

    const id = typeof parsed.data.id === "string" ? parsed.data.id.trim() : "";
    if (!id) {
      issues.push({ file: relFromRepo, message: "Missing non-empty `id`." });
    } else if (seenIds.has(id)) {
      issues.push({
        file: relFromRepo,
        message: `Duplicate rule id \`${id}\` (already defined in ${seenIds.get(id)}).`,
      });
    } else {
      seenIds.set(id, relFromRepo);
    }

    const summary =
      typeof parsed.data.summary === "string" ? parsed.data.summary.trim() : "";
    if (!summary) {
      issues.push({
        file: relFromRepo,
        message: "Missing non-empty `summary`.",
      });
    }

    if (parsed.data.severity !== undefined) {
      const sev =
        typeof parsed.data.severity === "string"
          ? parsed.data.severity.trim()
          : "";
      if (!VALID_SEVERITIES.has(sev as RuleSeverity)) {
        issues.push({
          file: relFromRepo,
          message: `Invalid severity \`${String(parsed.data.severity)}\` (expected invariant | guideline | adr).`,
        });
      }
    }

    if (parsed.data.paths !== undefined && !Array.isArray(parsed.data.paths)) {
      issues.push({
        file: relFromRepo,
        message: "`paths` must be an array of glob strings.",
      });
    }
  }

  return {
    valid: issues.length === 0,
    checked,
    issues,
  };
}
