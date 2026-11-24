import fs from "node:fs";
import path from "node:path";
import {
  AGENTS_FILE,
  CONFIG_FILE,
  keelPath,
  RULES_DIR,
  SKILLS_DIR,
  TASKS_DIR,
  toPosixPath,
} from "./paths.js";
import {
  AGENTS_END_MARKER,
  AGENTS_START_MARKER,
  getStandardSkills,
  renderAgentsBlock,
  renderDefaultConfig,
  renderRulesIndex,
} from "../templates/assets.js";
import type { VerifyGateConfig } from "./types.js";

export interface InitOptions {
  force?: boolean;
  skipSkills?: boolean;
  detectGates?: boolean;
}

export interface InitResult {
  created: string[];
  updated: string[];
  skipped: string[];
  detectedGates: VerifyGateConfig[];
}

function detectPackageManager(repoRoot: string): "pnpm" | "yarn" | "bun" | "npm" {
  if (fs.existsSync(path.join(repoRoot, "pnpm-lock.yaml"))) return "pnpm";
  if (fs.existsSync(path.join(repoRoot, "yarn.lock"))) return "yarn";
  if (
    fs.existsSync(path.join(repoRoot, "bun.lock")) ||
    fs.existsSync(path.join(repoRoot, "bun.lockb"))
  ) {
    return "bun";
  }
  return "npm";
}

export function detectProjectGates(repoRoot: string): VerifyGateConfig[] {
  const gates: VerifyGateConfig[] = [];

  const pkgPath = path.join(repoRoot, "package.json");
  if (fs.existsSync(pkgPath)) {
    try {
      const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf-8")) as {
        scripts?: Record<string, string>;
      };
      const scripts = pkg.scripts ?? {};
      const pm = detectPackageManager(repoRoot);
      const runPrefix = pm === "npm" ? "npm run" : pm;

      for (const name of ["typecheck", "lint", "test"]) {
        if (typeof scripts[name] === "string" && scripts[name]!.trim().length > 0) {
          gates.push({ name, command: `${runPrefix} ${name}` });
        }
      }
      return gates;
    } catch {
      // Ignore malformed package.json
    }
  }

  if (fs.existsSync(path.join(repoRoot, "Cargo.toml"))) {
    gates.push({ name: "check", command: "cargo check" });
    gates.push({ name: "test", command: "cargo test" });
    return gates;
  }

  if (fs.existsSync(path.join(repoRoot, "go.mod"))) {
    gates.push({ name: "vet", command: "go vet ./..." });
    gates.push({ name: "test", command: "go test ./..." });
    return gates;
  }

  if (fs.existsSync(path.join(repoRoot, "pyproject.toml"))) {
    gates.push({ name: "test", command: "pytest" });
    return gates;
  }

  return gates;
}

function writeIfMissingOrForced(
  repoRoot: string,
  fullPath: string,
  content: string,
  force: boolean,
  result: InitResult,
): void {
  const rel = toPosixPath(path.relative(repoRoot, fullPath));
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });

  if (fs.existsSync(fullPath)) {
    if (!force) {
      result.skipped.push(rel);
      return;
    }
    fs.writeFileSync(fullPath, content, "utf-8");
    result.updated.push(rel);
    return;
  }

  fs.writeFileSync(fullPath, content, "utf-8");
  result.created.push(rel);
}

function upsertAgentsFile(repoRoot: string, result: InitResult): void {
  const agentsPath = path.join(repoRoot, AGENTS_FILE);
  const block = renderAgentsBlock();

  if (!fs.existsSync(agentsPath)) {
    fs.writeFileSync(agentsPath, `${block}\n`, "utf-8");
    result.created.push(AGENTS_FILE);
    return;
  }

  const existing = fs.readFileSync(agentsPath, "utf-8");
  const startIdx = existing.indexOf(AGENTS_START_MARKER);
  const endIdx = existing.indexOf(AGENTS_END_MARKER);

  if (startIdx !== -1 && endIdx !== -1 && endIdx > startIdx) {
    const before = existing.slice(0, startIdx);
    const after = existing.slice(endIdx + AGENTS_END_MARKER.length);
    const next = `${before}${block}${after}`;
    if (next === existing) {
      result.skipped.push(AGENTS_FILE);
    } else {
      fs.writeFileSync(agentsPath, next, "utf-8");
      result.updated.push(AGENTS_FILE);
    }
    return;
  }

  const separator = existing.endsWith("\n") ? "\n" : "\n\n";
  fs.writeFileSync(agentsPath, `${existing}${separator}${block}\n`, "utf-8");
  result.updated.push(AGENTS_FILE);
}

export function initRepo(
  repoRoot: string,
  options: InitOptions = {},
): InitResult {
  const force = options.force ?? false;
  const detectGates = options.detectGates ?? true;
  const detectedGates = detectGates ? detectProjectGates(repoRoot) : [];

  const result: InitResult = {
    created: [],
    updated: [],
    skipped: [],
    detectedGates,
  };

  writeIfMissingOrForced(
    repoRoot,
    keelPath(repoRoot, CONFIG_FILE),
    renderDefaultConfig(detectedGates),
    force,
    result,
  );

  writeIfMissingOrForced(
    repoRoot,
    keelPath(repoRoot, RULES_DIR, "index.md"),
    renderRulesIndex(),
    force,
    result,
  );

  writeIfMissingOrForced(
    repoRoot,
    keelPath(repoRoot, TASKS_DIR, ".gitkeep"),
    "",
    false,
    result,
  );

  upsertAgentsFile(repoRoot, result);

  if (!options.skipSkills) {
    for (const skill of getStandardSkills()) {
      const skillPath = path.join(
        repoRoot,
        SKILLS_DIR,
        skill.dirName,
        "SKILL.md",
      );
      writeIfMissingOrForced(repoRoot, skillPath, skill.content, force, result);
    }
  }

  return result;
}
