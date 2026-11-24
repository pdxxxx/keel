use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_keel");

struct Repo {
    temp: TempDir,
    root: PathBuf,
    home: PathBuf,
    state: PathBuf,
}
impl Repo {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo 文");
        let home = temp.path().join("home");
        let state = temp.path().join("state");
        fs::create_dir(&home).unwrap();
        fs::create_dir(&root).unwrap();
        Self {
            temp,
            root,
            home,
            state,
        }
    }
    fn exec(&self, args: &[&str]) -> Output {
        Command::new(BIN)
            .current_dir(&self.home)
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .arg("--json")
            .env("USERPROFILE", &self.home)
            .env("HOME", &self.home)
            .env_remove("KEEL_MEM_DIRS")
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let out = self.exec(args);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.stderr.is_empty());
        serde_json::from_slice(&out.stdout).unwrap()
    }
    fn fail(&self, args: &[&str]) -> Output {
        let out = self.exec(args);
        assert!(!out.status.success(), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.starts_with("keel: ") && err.lines().count() == 1,
            "{err}"
        );
        out
    }
    fn st(&self) -> &str {
        self.state.to_str().unwrap()
    }
    fn put(&self, name: &str, contents: impl AsRef<[u8]>) {
        put(&self.root.join(name), contents);
    }
    fn read(&self, name: &str) -> Vec<u8> {
        fs::read(self.root.join(name)).unwrap()
    }
}
fn put(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}
fn revision(root: &Path, parent: Option<&str>, title: &str) -> String {
    let tree = git(root, &["write-tree"]);
    let mut args = vec![
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "commit-tree",
        &tree,
        "-m",
        title,
    ];
    if let Some(parent) = parent {
        args.extend(["-p", parent]);
    }
    let rev = git(root, &args);
    git(root, &["update-ref", "HEAD", &rev]);
    rev
}

#[test]
fn init_detects_all_manifests_and_only_explicit_reset_replaces_config() {
    let repo = Repo::new();
    repo.put(
        "package.json",
        r#"{"scripts":{"lint":"lint","test":"test"}}"#,
    );
    repo.put("pnpm-lock.yaml", "");
    for name in ["Cargo.toml", "go.mod", "pyproject.toml"] {
        repo.put(name, "");
    }
    let output = repo.ok(&["init"]);
    assert_eq!(output["created"].as_array().unwrap().len(), 9);
    let cmds = output["detectedGates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|gate| gate["command"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        cmds,
        [
            "pnpm lint",
            "pnpm test",
            "cargo check",
            "cargo test",
            "go vet ./...",
            "go test ./...",
            "pytest"
        ]
    );
    assert_eq!(
        repo.read(".agents/skills/keel-migrate/SKILL.md"),
        include_bytes!("../.agents/skills/keel-migrate/SKILL.md")
    );
    repo.put(".keel/config.yaml", "# mine\nverify: {gates: []}\n");
    repo.ok(&["init", "--force"]);
    assert_eq!(
        repo.read(".keel/config.yaml"),
        b"# mine\nverify: {gates: []}\n"
    );
    repo.ok(&["init", "--reset-config", "--no-detect"]);
    assert!(
        !String::from_utf8(repo.read(".keel/config.yaml"))
            .unwrap()
            .contains("# mine")
    );
    assert_eq!(repo.ok(&["doctor"])["errors"], json!([]));
}

#[test]
fn update_is_dry_run_aware_idempotent_and_touches_only_managed_templates() {
    let repo = Repo::new();
    repo.fail(&["update"]);
    repo.ok(&["init", "--no-skills"]);
    let before =
        "\u{feff}TEAM\r\n<!-- KEEL:START -->\r\nold\r\n<!-- KEEL:END -->\r\nTail without newline";
    repo.put("AGENTS.md", before);
    repo.put(".keel/tasks/task/spec.md", "task");
    repo.put(".keel/rules/user.md", "manual");
    repo.put(".keel/rules/index.md", "custom index");
    let config = repo.read(".keel/config.yaml");
    let preview = repo.ok(&["update", "--dry-run"]);
    assert_eq!(preview["created"].as_array().unwrap().len(), 5);
    assert_eq!(repo.read("AGENTS.md"), before.as_bytes());
    assert!(!repo.root.join(".agents").exists());
    repo.ok(&["update", "--no-skills"]);
    assert!(!repo.root.join(".agents").exists());
    repo.ok(&["update"]);
    let next = String::from_utf8(repo.read("AGENTS.md")).unwrap();
    assert!(next.starts_with("\u{feff}TEAM\r\n<!-- KEEL:START -->"));
    assert!(next.ends_with("<!-- KEEL:END -->\r\nTail without newline"));
    assert!(next.contains("`root`"));
    assert_eq!(repo.ok(&["update"])["updated"], json!([]));
    assert_eq!(repo.read(".keel/tasks/task/spec.md"), b"task");
    assert_eq!(repo.read(".keel/rules/user.md"), b"manual");
    assert_eq!(repo.read(".keel/config.yaml"), config);
}

#[test]
fn malformed_markers_never_partially_initialize_or_upgrade_a_repository() {
    let repo = Repo::new();
    let start = "<!-- KEEL:START -->";
    let end = "<!-- KEEL:END -->";
    for text in [
        start.into(),
        end.into(),
        format!("{end}{start}"),
        format!("{start}{start}{end}{end}"),
        format!("{start}{end}{start}{end}"),
        "<!-- KEEL:STRT -->".into(),
    ] {
        repo.put("AGENTS.md", &text);
        let out = repo.fail(&["init", "--force", "--reset-config"]);
        assert!(out.stdout.is_empty());
        assert_eq!(repo.read("AGENTS.md"), text.as_bytes());
        assert!(!repo.root.join(".keel").exists());
    }
    repo.put(".keel/config.yaml", "version: 1");
    repo.put(".keel/rules/index.md", "untouched");
    repo.fail(&["update"]);
    assert_eq!(repo.read(".keel/rules/index.md"), b"untouched");
    assert!(!repo.root.join(".agents").exists());
}

#[test]
fn doctor_collects_all_config_and_rule_problems_and_verify_fails_closed() {
    let repo = Repo::new();
    let absent: Value = serde_json::from_slice(&repo.fail(&["doctor"]).stdout).unwrap();
    assert!(absent["errors"].as_array().unwrap().len() >= 5);
    repo.ok(&["init"]);
    repo.put(".keel/config.yaml","verify:\n  gates:\n    - {name: x, command: 'echo bad > ran'}\n    - {name: x, cmd: ignored, command: ' ', timeoutMs: 30s, maxOutputChars: -1, paths: [false]}\nmemory: {sources: 42}\nunknown: true");
    repo.put(
        ".keel/rules/stale.md",
        "---\nid: stale\nsummary: lonely\npaths: [missing/**]\n---\n",
    );
    repo.put(".keel/rules/bad.md", "# missing");
    let report: Value = serde_json::from_slice(&repo.fail(&["doctor"]).stdout).unwrap();
    let errors = report["errors"].to_string();
    for part in [
        ".cmd",
        "timeoutMs",
        "maxOutputChars",
        ".command",
        "duplicate gate",
        ".unknown",
        "memory.sources",
        ".paths",
        "frontmatter",
    ] {
        assert!(errors.contains(part), "{errors}");
    }
    assert_eq!(report["warnings"].as_array().unwrap().len(), 1);
    assert!(repo.fail(&["verify"]).stdout.is_empty());
    assert!(!repo.root.join("ran").exists());
    repo.put(".keel/config.yaml", "version: 1");
    fs::remove_file(repo.root.join(".keel/rules/bad.md")).unwrap();
    assert_eq!(repo.ok(&["doctor"])["errors"], json!([]));
    repo.fail(&["rules", "check", "--strict"]);
    repo.ok(&["rules", "check", "--stale"]);
}

#[test]
fn tasks_and_rules_round_trip_via_real_cli_and_reject_bad_ids() {
    let repo = Repo::new();
    let rule = repo.ok(&[
        "rules",
        "add",
        "My Rule",
        "-s",
        "a summary",
        "--severity",
        "adr",
        "--paths",
        "src/**",
        "--body",
        "why",
    ]);
    assert_eq!(rule["id"], "my-rule");
    assert_eq!(repo.ok(&["rules", "list"]), json!([rule]));
    assert_eq!(
        repo.ok(&["rules", "match", "src/file", "--no-global"])["matched"],
        json!([rule])
    );
    let created = repo.ok(&["task", "create", "A task", "--slug", "Unique"]);
    let id = created["id"].as_str().unwrap();
    assert!(
        repo.ok(&["task", "show", id])["spec"]
            .as_str()
            .unwrap()
            .contains("A task")
    );
    repo.ok(&["task", "done", id]);
    assert_eq!(
        repo.ok(&["task", "list", "--status", "done"])
            .as_array()
            .unwrap()
            .len(),
        1
    );
    repo.ok(&["task", "reopen", id]);
    repo.ok(&["task", "archive", id]);
    assert_eq!(repo.ok(&["task", "list"]), json!([]));
    assert_eq!(repo.ok(&["task", "list", "--all"])[0]["status"], "done");
    repo.ok(&["task", "show", &format!("archive/{id}")]);
    for id in ["../outside", "C:/tmp/task", "/abs", r"..\other"] {
        for command in ["show", "done", "archive"] {
            assert!(repo.fail(&["task", command, id]).stdout.is_empty());
        }
    }
}

#[test]
fn nested_monorepo_scopes_base_staged_untracked_gates_and_hotspots() {
    let mut repo = Repo::new();
    let top = repo.root.clone();
    let sub = top.join("packages/app");
    git(&top, &["init", "-q"]);
    repo.put("packages/app/src/name.rs", "original");
    repo.put("outside.txt", "outside");
    git(&top, &["add", "."]);
    let first = revision(&top, None, "initial");
    repo.put("packages/app/src/name.rs", "updated");
    repo.put("outside.txt", "also changed");
    git(&top, &["add", "."]);
    revision(&top, Some(&first), "fix: regression");
    repo.root = sub;
    repo.ok(&["init", "--no-skills"]);
    repo.ok(&["rules", "add", "api", "-s", "scoped", "-p", "src/**"]);
    repo.put(".keel/config.yaml","verify: {gates: [{name: api, command: echo api, paths: [src/**]}, {name: web, command: echo web, paths: [web/**]}]}");
    repo.put("web/staged", "staged");
    git(&top, &["add", "packages/app/web"]);
    repo.put("untracked", "new");
    assert_eq!(
        repo.ok(&["rules", "match", "--base", &first])["targets"],
        json!(["src/name.rs"])
    );
    assert_eq!(
        repo.ok(&["rules", "match", "--staged"])["targets"],
        json!(["web/staged"])
    );
    assert_eq!(repo.ok(&["verify", "--staged"])["skipped"], json!(["api"]));
    assert_eq!(
        repo.ok(&["verify", "--base", &first])["skipped"],
        json!(["web"])
    );
    assert_eq!(
        repo.ok(&["verify", "--git", "-g", "api", "web"])["gates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    repo.fail(&["verify", "--base", "missing-ref"]);
    assert!(
        repo.ok(&["rules", "match", "--git"])["targets"]
            .as_array()
            .unwrap()
            .contains(&json!("untracked"))
    );
    let ranked = repo.ok(&["hotspots", "--limit", "1"]);
    assert_eq!(ranked[0]["file"], "src/name.rs");
    assert_eq!(ranked[0]["score"], 5);
}

#[test]
fn memory_filters_all_seven_sources_and_streams_gemini_without_partial_results() {
    let repo = Repo::new();
    repo.put(".keel/config.yaml", "memory: {sources: [custom]}");
    repo.put(".keel/rules/document.md", "project needle");
    repo.put("custom/note.md", "other needle");
    let root = &repo.root;
    let query = "needle";
    for (file, frame) in [
        (
            ".claude/projects/a/a.jsonl",
            json!({"role":"user","text":query,"cwd":root}),
        ),
        (
            ".codex/sessions/a.jsonl",
            json!({"payload":{"role":"user","text":query,"cwd":root}}),
        ),
        (
            ".local/share/opencode/chat.jsonl",
            json!({"role":"assistant","text":query}),
        ),
        (
            ".gemini/antigravity/brain/id/.system_generated/logs/transcript.jsonl",
            json!({"type":"USER_INPUT","content":query,"created_at":"now","workspace":root}),
        ),
    ] {
        put(&repo.home.join(file), format!("bad\n{frame}\n"));
    }
    let chat = repo.home.join(".gemini/tmp/proj/chats");
    put(
        &chat.join("yes.json"),
        format!(
            r#"{{"messages":[{{"type":"gemini","content":{{"parts":[{{"text":"needle"}}]}}}},{{"type":"model","text":"second"}}],"metadata":{{"cwd":{}}}}}"#,
            json!(root)
        ),
    );
    put(
        &chat.join("bad.json"),
        r#"{"messages":[{"type":"user","text":"needle"}],"late": "#,
    );
    put(
        &chat.join("foreign.json"),
        json!({"cwd":repo.home,"messages":[{"type":"user","text":"needle"}]}).to_string(),
    );
    assert_eq!(repo.ok(&["mem", "sources"]).as_array().unwrap().len(), 7);
    assert_eq!(
        repo.ok(&["mem", "sources", "--source", "gemini"])[0]["sessionCount"],
        2
    );
    let sessions = repo.ok(&["mem", "sessions", "--scope", "project"]);
    assert_eq!(sessions.as_array().unwrap().len(), 6);
    let hits = repo.ok(&["mem", "search", query, "--scope", "all"]);
    assert_eq!(hits.as_array().unwrap().len(), 8);
    assert_eq!(hits[0]["score"], 15.5);
    let gemini = repo.ok(&[
        "mem", "search", query, "--source", "gemini", "--scope", "project",
    ]);
    assert_eq!(gemini[0]["score"], 10.5);
    let id = gemini[0]["sessionId"].as_str().unwrap();
    let detail = repo.ok(&["mem", "show", &id[id.len() - 10..], "-n", "1"]);
    assert_eq!(detail["summary"]["turnCount"], 2);
    assert_eq!(
        detail["turns"],
        json!([{"role":"assistant","text":"needle"}])
    );
    repo.fail(&["mem", "show", "not-found"]);
}

#[test]
fn verify_limits_unicode_output_and_marks_only_timeouts() {
    let repo = Repo::new();
    repo.put("script.cjs", "process.stdout.write('1234🚀文end')");
    repo.put(".keel/config.yaml","verify:\n  gates:\n  - {name: bounded, command: node script.cjs, maxOutputChars: 5}\n  - {name: watch, command: 'node -e \"setTimeout(()=>{},4000)\"', timeoutMs: 300}\n  - {name: last, command: echo last}\n");
    let report: Value =
        serde_json::from_slice(&repo.fail(&["verify", "--fail-fast"]).stdout).unwrap();
    let gates = report["gates"].as_array().unwrap();
    assert_eq!(gates.len(), 2);
    assert_eq!(gates[0]["output"], "[truncated: omitted 4 chars]\n🚀文end");
    assert!(gates[0].get("timedOut").is_none());
    assert_eq!(gates[1]["exitCode"], 124);
    assert_eq!(gates[1]["timedOut"], true);
    assert_eq!(gates[1]["output"], "[timed out after 300ms]");
}

#[test]
fn migrate_scan_plan_and_cleanup_preserve_mixed_hooks_and_restore_exact_bytes() {
    use sha2::{Digest, Sha256};
    let repo = Repo::new();
    let legacy = "<!-- TRELLIS:START -->\r\nlegacy\r\n<!-- TRELLIS:END -->\r\n";
    let keep = "<!-- KEEL:START -->\r\nactive\r\n<!-- KEEL:END -->\r\n";
    let agents = format!("Prose\r\n{legacy}{keep}{legacy}Tail");
    for (name, text) in [
        ("AGENTS.md", agents.as_str()),
        ("CLAUDE.md", &format!("Hi\r\n{legacy}Bye")),
        (".claude/agents/trellis-old/AGENT.md", "old"),
        (".codex/agents/trellis-old.toml", "old"),
        (".cursor/commands/trellis-old.md", "old"),
        (".agents/skills/trellis-old/SKILL.md", "old"),
        (".agents/skills/team/SKILL.md", "keep"),
        (".cursor/hooks/check.sh", "run trellis"),
        (".claude/hooks/init.sh", "trellis init"),
        (".codex/hooks/nested/tool.py", "import trellis"),
        (".cursorrules", "old"),
        (
            ".cursor/rules/r.mdc",
            "---\nglobs: 'src/*.{ts,tsx}, web/**'\n---\n",
        ),
        (".trellis/tasks/archive/one/spec.md", "archived"),
        (".windsurfrules", "source"),
        (".specify/memory/constitution.md", "source"),
        ("_bmad/templates/one.md", "source"),
        ("docs/adr/one.md", "memory"),
        (".github/copilot-instructions.md", "keep"),
        ("package.json", r#"{"scripts":{"lint":"eslint ."}}"#),
    ] {
        repo.put(name, text);
    }
    repo.put(".trellis/data.bin", [0, 22, 255]);
    fs::create_dir_all(repo.root.join(".trellis/empty/deep")).unwrap();
    let settings = [
        ".gemini/settings.json",
        ".codex/hooks.json",
        ".cursor/hooks.json",
        ".claude/settings.json",
    ];
    let preserve =
        json!({"command":"vendor/hooks/check.sh","description":"Trellis prose is not a command"});
    for file in settings {
        repo.put(file,json!({"theme":"trellis","hooks":{"Stop":[{"hooks":[{"command":"python .codex/hooks/nested/tool.py"},preserve]}],"Other":[{"command":".cursor/hooks/check.sh"},preserve]}}).to_string());
    }
    let scan = repo.ok(&["migrate", "scan"]);
    let items = scan["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .any(|item| item["globs"] == json!(["src/*.{ts,tsx}", "web/**"]))
    );
    let plan = repo.ok(&["migrate", "plan"]);
    assert_eq!(plan["suggestedGates"][0]["command"], "npm run lint");
    assert_eq!(
        plan["suggestedMemorySources"],
        json!([".trellis/tasks/archive", "docs/adr"])
    );
    let preview = repo.ok(&["migrate", "cleanup", "--state-dir", repo.st(), "--dry-run"]);
    assert!(!repo.state.exists());
    let originals: Vec<_> = preview["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let name = entry["relativePath"].as_str().unwrap();
            (name, repo.read(name))
        })
        .collect();
    let applied = repo.ok(&["migrate", "cleanup", "--state-dir", repo.st()]);
    assert_eq!(preview["entries"], applied["entries"]);
    assert!(!repo.root.join(".trellis").exists());
    assert!(!repo.root.join(".agents/skills/trellis-old").exists());
    assert_eq!(repo.read(".agents/skills/team/SKILL.md"), b"keep");
    assert_eq!(
        repo.read("AGENTS.md"),
        format!("Prose\r\n{keep}Tail").as_bytes()
    );
    assert_eq!(repo.read("CLAUDE.md"), b"Hi\r\nBye");
    for name in settings {
        assert_eq!(
            serde_json::from_slice::<Value>(&repo.read(name)).unwrap(),
            json!({"theme":"trellis","hooks":{"Stop":[{"hooks":[preserve]}],"Other":[preserve]}})
        );
    }
    let store = Path::new(applied["snapshotDir"].as_str().unwrap());
    for (entry, (name, bytes)) in applied["entries"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&originals)
    {
        assert_eq!(fs::read(store.join("files").join(name)).unwrap(), *bytes);
        assert_eq!(
            entry["originalSha256"],
            format!("{:x}", Sha256::digest(bytes))
        );
    }
    repo.ok(&["migrate", "rollback", "--state-dir", repo.st(), "--dry-run"]);
    assert!(store.exists());
    repo.put("CLAUDE.md", "post cleanup");
    repo.fail(&["migrate", "rollback", "--state-dir", repo.st()]);
    assert!(!repo.root.join(".cursorrules").exists());
    repo.ok(&["migrate", "rollback", "--state-dir", repo.st(), "--force"]);
    for (name, bytes) in originals {
        assert_eq!(repo.read(name), bytes);
    }
    assert!(repo.root.join(".trellis/empty/deep").is_dir());
    assert!(!store.exists());
}

#[test]
fn corrupt_late_backup_and_tampered_manifest_are_never_force_restored() {
    let repo = Repo::new();
    repo.put(".cursorrules", "first");
    repo.put(".trellis/last", "last");
    let report = repo.ok(&["migrate", "cleanup", "--state-dir", repo.st()]);
    let store = Path::new(report["snapshotDir"].as_str().unwrap());
    put(&store.join("files/.trellis/last"), "tampered");
    repo.fail(&["migrate", "rollback", "--state-dir", repo.st(), "--force"]);
    assert!(!repo.root.join(".cursorrules").exists());
    assert!(!repo.root.join(".trellis").exists());
    put(&store.join("files/.trellis/last"), "last");
    let manifest = store.join("manifest.json");
    let before = fs::read(&manifest).unwrap();
    for path in ["../outside", "C:/windows", ".git/config"] {
        let mut data: Value = serde_json::from_slice(&before).unwrap();
        data["entries"][1]["relativePath"] = json!(path);
        put(&manifest, data.to_string());
        repo.fail(&["migrate", "rollback", "--state-dir", repo.st(), "--force"]);
        assert!(!repo.root.join(".cursorrules").exists());
    }
    put(&manifest, before);
    repo.ok(&["migrate", "rollback", "--state-dir", repo.st()]);
}

#[test]
fn ablate_status_restore_preview_and_conflict_handling_use_the_same_external_store() {
    let repo = Repo::new();
    repo.put("AGENTS.md", "team body without newline");
    repo.ok(&["init"]);
    let before = repo.read("AGENTS.md");
    let config = repo.read(".keel/config.yaml");
    let copy = repo.temp.path().join("hardlink");
    fs::hard_link(repo.root.join("AGENTS.md"), &copy).unwrap();
    assert_eq!(
        repo.ok(&["ablate", "status", "--state-dir", repo.st()])["ablated"],
        false
    );
    repo.ok(&["ablate", "--dry-run", "--state-dir", repo.st()]);
    assert!(!repo.state.exists());
    repo.fail(&[
        "ablate",
        "--state-dir",
        repo.root.join("internal").to_str().unwrap(),
    ]);
    assert!(!repo.root.join("internal").exists());
    let output = repo.ok(&["ablate", "--state-dir", repo.st()]);
    assert!(output["entries"].as_array().unwrap().len() >= 9);
    assert!(!repo.root.join(".keel").exists());
    assert!(!repo.root.join(".agents/skills/keel-plan").exists());
    assert_eq!(fs::read(copy).unwrap(), before);
    let after = repo.read("AGENTS.md");
    assert!(
        String::from_utf8(after.clone())
            .unwrap()
            .contains("team body")
    );
    assert!(!String::from_utf8(after.clone()).unwrap().contains("KEEL:"));
    let copy = repo.temp.path().join("hardlink-during");
    fs::hard_link(repo.root.join("AGENTS.md"), &copy).unwrap();
    assert_eq!(
        repo.ok(&["ablate", "status", "--state-dir", repo.st()])["ablated"],
        true
    );
    repo.ok(&["restore", "--dry-run", "--state-dir", repo.st()]);
    assert!(!repo.root.join(".keel").exists());
    repo.put(".keel/config.yaml", "conflict");
    repo.fail(&["restore", "--state-dir", repo.st()]);
    assert_eq!(repo.read(".keel/config.yaml"), b"conflict");
    assert!(!repo.root.join(".agents/skills/keel-plan").exists());
    repo.ok(&["restore", "--force", "--state-dir", repo.st()]);
    assert_eq!(repo.read(".keel/config.yaml"), config);
    assert_eq!(repo.read("AGENTS.md"), before);
    assert_eq!(fs::read(copy).unwrap(), after);
    assert_eq!(
        repo.ok(&["ablate", "status", "--state-dir", repo.st()])["ablated"],
        false
    );
}

#[test]
fn malformed_migration_inputs_leave_all_conflicts_and_state_untouched() {
    let repo = Repo::new();
    repo.put(".cursorrules", "keep");
    for invalid in [
        "<!-- TRELLIS:START -->",
        "<!-- KEEL:START --><!-- TRELLIS:START --><!-- KEEL:END --><!-- TRELLIS:END -->",
    ] {
        repo.put("CLAUDE.md", invalid);
        repo.fail(&["migrate", "cleanup", "--state-dir", repo.st()]);
        assert_eq!(repo.read("CLAUDE.md"), invalid.as_bytes());
        assert_eq!(repo.read(".cursorrules"), b"keep");
        assert!(!repo.state.exists());
    }
    repo.put("CLAUDE.md", "regular");
    repo.put(".codex/hooks.json", "{trellis");
    repo.fail(&["migrate", "cleanup", "--state-dir", repo.st()]);
    assert!(!repo.state.exists());
    assert_eq!(repo.read(".cursorrules"), b"keep");
}

fn link_dir(link: &Path, target: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let out=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command", "New-Item -ItemType Junction -Path $env:KEEL_LINK -Target $env:KEEL_TARGET -ErrorAction Stop | Out-Null"]).env("KEEL_LINK",link).env("KEEL_TARGET",target).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn snapshots_reject_linked_workspace_state_and_backup_paths_even_with_force() {
    let repo = Repo::new();
    let outside = repo.temp.path().join("outside");
    put(&outside.join("file"), "untouched");
    repo.put(".trellis/file", "saved");
    link_dir(&repo.root.join(".trellis/link"), &outside);
    repo.fail(&["migrate", "cleanup", "--state-dir", repo.st()]);
    assert!(!repo.state.exists());
    assert_eq!(repo.read(".trellis/file"), b"saved");
    #[cfg(windows)]
    fs::remove_dir(repo.root.join(".trellis/link")).unwrap();
    #[cfg(unix)]
    fs::remove_file(repo.root.join(".trellis/link")).unwrap();
    link_dir(&repo.temp.path().join("state-alias"), &outside);
    repo.fail(&[
        "migrate",
        "cleanup",
        "--state-dir",
        repo.temp.path().join("state-alias").to_str().unwrap(),
    ]);
    let applied = repo.ok(&["migrate", "cleanup", "--state-dir", repo.st()]);
    let store = Path::new(applied["snapshotDir"].as_str().unwrap());
    fs::rename(store.join("files/.trellis"), store.join("saved")).unwrap();
    link_dir(&store.join("files/.trellis"), &outside);
    repo.fail(&["migrate", "rollback", "--state-dir", repo.st(), "--force"]);
    assert!(!repo.root.join(".trellis").exists());
    assert_eq!(fs::read(outside.join("file")).unwrap(), b"untouched");
}

#[test]
fn doctor_reports_git_missing_from_path_as_a_health_error() {
    let repo = Repo::new();
    repo.ok(&["init"]);
    let out = Command::new(BIN)
        .arg("--root")
        .arg(&repo.root)
        .args(["doctor", "--json"])
        .env("PATH", &repo.home)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["errors"], json!(["git is unavailable on PATH"]));
}

#[cfg(unix)]
#[test]
fn snapshot_refuses_unrepresentable_filenames_instead_of_backup_under_an_alias() {
    #[cfg(not(target_os = "macos"))]
    use std::os::unix::ffi::OsStringExt;
    let repo = Repo::new();
    repo.put(".trellis/sub/name", "keep");
    #[allow(unused_mut)]
    let mut names = vec![std::ffi::OsString::from(r"sub\name")];
    #[cfg(not(target_os = "macos"))]
    names.push(std::ffi::OsString::from_vec(vec![255, b'x']));
    for name in names {
        let path = repo.root.join(".trellis").join(name);
        put(&path, "separate");
        repo.fail(&["migrate", "cleanup", "--state-dir", repo.st()]);
        assert!(!repo.state.exists());
        assert_eq!(repo.read(".trellis/sub/name"), b"keep");
        assert_eq!(fs::read(&path).unwrap(), b"separate");
        fs::remove_file(path).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn rollback_restores_original_unix_permissions_and_empty_directories() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new();
    repo.put(".trellis/run", "exec");
    let dir = repo.root.join(".trellis/empty");
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o711)).unwrap();
    fs::set_permissions(
        repo.root.join(".trellis/run"),
        fs::Permissions::from_mode(0o751),
    )
    .unwrap();
    repo.ok(&["migrate", "cleanup", "--state-dir", repo.st()]);
    repo.ok(&["migrate", "rollback", "--state-dir", repo.st()]);
    assert_eq!(
        fs::metadata(dir).unwrap().permissions().mode() & 0o777,
        0o711
    );
    assert_eq!(
        fs::metadata(repo.root.join(".trellis/run"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o751
    );
}
