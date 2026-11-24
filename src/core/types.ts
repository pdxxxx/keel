export type RuleSeverity = "invariant" | "guideline" | "adr";

export interface RuleEntry {
  id: string;
  file: string;
  summary: string;
  severity: RuleSeverity;
  paths: string[];
  global: boolean;
}

export interface RuleIssue {
  file: string;
  message: string;
}

export interface RuleValidationReport {
  valid: boolean;
  checked: number;
  issues: RuleIssue[];
  warnings?: RuleIssue[];
}

export type TaskStatus = "open" | "done";

export interface TaskMeta {
  id: string;
  title: string;
  status: TaskStatus;
  createdAt: string;
  updatedAt: string;
}

export interface TaskRecord extends TaskMeta {
  dir: string;
  specFile: string;
}

export interface VerifyGateConfig {
  name: string;
  command: string;
  cwd?: string;
  paths?: string[];
}

export interface MemoryConfig {
  sources: string[];
}

export interface KeelConfig {
  version: number;
  verify: {
    gates: VerifyGateConfig[];
  };
  memory: MemoryConfig;
}

export interface VerifyGateResult {
  name: string;
  command: string;
  passed: boolean;
  exitCode: number;
  durationMs: number;
  output: string;
}

export interface VerifyReport {
  passed: boolean;
  gates: VerifyGateResult[];
  skipped?: string[];
}

export interface MemoryTurn {
  role: "user" | "assistant" | "artifact";
  text: string;
  timestamp?: string;
}

export interface MemorySessionSummary {
  id: string;
  source: string;
  file: string;
  title: string;
  updatedAt: string;
  turnCount: number;
}

export interface MemorySearchHit {
  sessionId: string;
  source: string;
  file: string;
  role: MemoryTurn["role"];
  score: number;
  snippet: string;
  timestamp?: string;
}

export interface AblateManifestEntry {
  kind: "file" | "agents-block" | "config-edit";
  relativePath: string;
  originalSha256: string;
  postAblateSha256: string | null;
  backupRelPath: string;
  originalMode?: number;
}

export interface AblateManifest {
  version: 1;
  repoRoot: string;
  createdAt: string;
  entries: AblateManifestEntry[];
  directories?: string[];
  operation?: "migrate";
}
