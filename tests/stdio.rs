use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, File, FileTimes};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, UNIX_EPOCH};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_keel");
const TOOLS: [&str; 13] = [
    "keel_rules_list",
    "keel_rules_match",
    "keel_rules_check",
    "keel_task_list",
    "keel_task_create",
    "keel_task_show",
    "keel_verify",
    "keel_mem_search",
    "keel_mem_sessions",
    "keel_mem_show",
    "keel_rules_add",
    "keel_task_update",
    "keel_hotspots",
];

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    home: PathBuf,
    env_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("sample repo");
        let home = temp.path().join("fake home");
        let env_dir = temp.path().join("extra env notes");
        for path in [&root, &home, &env_dir] {
            fs::create_dir(path).unwrap();
        }
        Self {
            _temp: temp,
            root,
            home,
            env_dir,
        }
    }

    fn server(&self, cwd: &Path, explicit_root: bool) -> Server {
        self.launch(cwd, explicit_root, false)
    }

    fn launch(&self, cwd: &Path, explicit_root: bool, lock: bool) -> Server {
        let mut cmd = Command::new(BIN);
        cmd.arg("mcp");
        if lock {
            cmd.arg("--lock-root");
        }
        if explicit_root {
            cmd.arg("--root").arg(&self.root);
        }
        let mut child = cmd
            .current_dir(cwd)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("KEEL_MEM_DIRS", &self.env_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Server {
            input: child.stdin.take(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            next_id: 0,
        }
    }

    fn start(&self) -> Server {
        self.server(&self.home, true)
    }
}

struct Server {
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    child: Child,
    next_id: u32,
}

impl Server {
    fn send(&mut self, bytes: &[u8]) {
        let input = self.input.as_mut().unwrap();
        input.write_all(bytes).unwrap();
        input.flush().unwrap();
    }

    fn response(&mut self) -> Value {
        let mut line = String::new();
        assert!(
            self.output.read_line(&mut line).unwrap() > 0,
            "server exited before replying"
        );
        assert!(line.ends_with('\n'), "not a complete frame: {line:?}");
        let frame: Value = serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("invalid stdout frame: {line:?}: {error}"));
        assert_eq!(frame["jsonrpc"], "2.0");
        frame
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id.to_string();
        let msg = json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params});
        self.send(format!("{msg}\n").as_bytes());
        let response = self.response();
        assert_eq!(response["id"], id);
        response
    }

    fn text(&mut self, name: &str, args: Value, failed: bool) -> String {
        let response = self.request("tools/call", json!({"name":name,"arguments":args}));
        assert!(response.get("error").is_none(), "{response}");
        let result = &response["result"];
        assert_eq!(
            result.get("isError"),
            failed.then_some(&Value::Bool(true)),
            "{result}"
        );
        assert_eq!(result["content"].as_array().unwrap().len(), 1);
        assert_eq!(result["content"][0]["type"], "text");
        result["content"][0]["text"].as_str().unwrap().to_owned()
    }

    fn data(&mut self, name: &str, args: Value) -> Value {
        serde_json::from_str(&self.text(name, args, false)).unwrap()
    }

    fn finish(mut self) {
        drop(self.input.take());
        let mut extra = Vec::new();
        self.output.read_to_end(&mut extra).unwrap();
        assert!(
            extra.is_empty(),
            "unexpected stdout: {}",
            String::from_utf8_lossy(&extra)
        );
        assert!(self.child.wait().unwrap().success());
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        drop(self.input.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn put(path: &Path, content: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn set_mtime(path: &Path, seconds: u64) {
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(seconds)))
        .unwrap();
}

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .unwrap()
        .trim_end()
        .to_owned()
}

#[test]
fn handshake_catalog_blank_lines_notifications_and_rpc_errors() {
    let fixture = Fixture::new();
    let mut server = fixture.start();
    let init = server.request("initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}));
    assert_eq!(
        init["result"],
        json!({"serverInfo":{"name":"keel","version":env!("CARGO_PKG_VERSION")},"protocolVersion":"2024-11-05","capabilities":{"tools":{}}})
    );
    server.send(b"\r\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"method\":\"notifications/cancelled\",\"id\":7}\n");
    let list = server.request("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        TOOLS
    );
    let expected_keys: [&[&str]; 13] = [
        &["root"],
        &["git", "includeGlobal", "paths", "root"],
        &["root", "stale"],
        &["includeArchived", "root", "status"],
        &["root", "slug", "title"],
        &["id", "root"],
        &["failFast", "gates", "git", "root"],
        &["limit", "query", "root", "scope", "source"],
        &["limit", "root", "scope", "source"],
        &["limit", "root", "sessionId"],
        &[
            "body", "force", "id", "paths", "root", "severity", "summary",
        ],
        &["action", "id", "root"],
        &["commits", "limit", "root"],
    ];
    for (tool, keys) in tools.iter().zip(expected_keys) {
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            keys
        );
    }
    assert_eq!(
        tools[3]["inputSchema"]["properties"]["status"]["enum"],
        json!(["open", "done"])
    );
    assert_eq!(
        server.request("tools/call", json!({}))["error"],
        json!({"code":-32602,"message":"Missing tool name"})
    );
    assert_eq!(
        server.request("elsewhere", Value::Null)["error"],
        json!({"code":-32601,"message":"Method not found: elsewhere"})
    );
    assert_eq!(
        server.text("unknown", json!({}), true),
        "Unknown tool: unknown"
    );
    for (name, arg, args) in [
        ("keel_task_create", "title", json!({"title":"   "})),
        ("keel_task_show", "id", json!({"id":4})),
        ("keel_mem_search", "query", json!({})),
    ] {
        assert_eq!(
            server.text(name, args, true),
            format!("Missing required string argument: {arg}")
        );
    }
    server.send(b"bad json\r\n\xff\n{}\n");
    for code in [-32700, -32700, -32600] {
        let frame = server.response();
        assert_eq!(frame["id"], Value::Null);
        assert_eq!(frame["error"]["code"], code);
    }
    assert_eq!(server.request("ping", Value::Null)["result"], json!({}));
    server.send(b"{\"jsonrpc\":\"1.0\",\"id\":3,\"method\":\"ping\"}\n");
    assert_eq!(server.response()["error"]["code"], -32600);
    server.finish();
}

#[test]
fn scoped_rules_include_staged_unstaged_untracked_and_global() {
    let fixture = Fixture::new();
    let root = &fixture.root;
    git(root, &["init", "-q"]);
    put(&root.join("working.txt"), "base");
    put(&root.join("staged.txt"), "base");
    put(&root.join(".gitignore"), "ignored.txt\n");
    git(root, &["add", "."]);
    let tree = git(root, &["write-tree"]);
    let revision = git(
        root,
        &[
            "-c",
            "user.name=Keel Test",
            "-c",
            "user.email=test@example.invalid",
            "commit-tree",
            &tree,
            "-m",
            "fixture",
        ],
    );
    git(root, &["update-ref", "HEAD", &revision]);
    put(&root.join("staged.txt"), "staged");
    git(root, &["add", "staged.txt"]);
    put(&root.join("working.txt"), "working");
    put(&root.join("untracked space.txt"), "fresh");
    put(&root.join("ignored.txt"), "ignored");
    let dir = root.join(".keel/rules");
    put(&dir.join("index.md"), "# Not a rule");
    for (id, glob) in [
        ("direct", "src/**/test?.rs"),
        ("ignored", "ignored.txt"),
        ("staged", "staged.txt"),
        ("untracked", "untracked space.txt"),
        ("working", "working.txt"),
    ] {
        put(
            &dir.join(format!("{id}.md")),
            format!("---\nid: {id}\nsummary: {id}\npaths: ['{glob}']\n---\n"),
        );
    }
    put(
        &dir.join("global.md"),
        "---\nid: global\nsummary: Everywhere\nseverity: adr\n---\n",
    );
    let mut server = fixture.start();
    assert_eq!(
        server
            .data("keel_rules_list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let matched = server.data(
        "keel_rules_match",
        json!({"git":true,"paths":[".\\src\\test1.rs",null],"includeGlobal":false}),
    );
    assert_eq!(
        matched
            .as_array()
            .unwrap()
            .iter()
            .map(|rule| rule["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["direct", "staged", "untracked", "working"]
    );
    assert_eq!(
        server.data("keel_rules_match", json!({}))[0],
        json!({"id":"global","file":".keel/rules/global.md","summary":"Everywhere","severity":"adr","paths":[],"global":true})
    );
    assert_eq!(
        server.data("keel_rules_check", json!({})),
        json!({"valid":true,"checked":6,"issues":[]})
    );
    put(
        &dir.join("zzz.md"),
        "---\nid: global\nseverity: typo\npaths: 7\n---\n",
    );
    let report: Value =
        serde_json::from_str(&server.text("keel_rules_check", json!({}), true)).unwrap();
    let issues = report["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 4);
    assert_eq!(issues[0]["file"], ".keel/rules/zzz.md");
    assert_eq!(
        issues[0]["message"],
        "Duplicate rule id `global` (already defined in .keel/rules/global.md)."
    );
    assert_eq!(issues[1]["message"], "Missing non-empty `summary`.");
    assert_eq!(
        issues[2]["message"],
        "Invalid severity `typo` (expected invariant | guideline | adr)."
    );
    assert_eq!(
        issues[3]["message"],
        "`paths` must be an array of glob strings."
    );
    server.finish();
}

#[test]
fn tasks_round_trip_archive_filter_and_root_discovery() {
    let fixture = Fixture::new();
    let root = &fixture.root;
    fs::create_dir(root.join(".keel")).unwrap();
    let nested = root.join("subdir/with-git");
    fs::create_dir_all(&nested).unwrap();
    put(&nested.join(".git"), "gitdir: unused");
    let mut server = fixture.server(&nested, false);
    let created = server.data(
        "keel_task_create",
        json!({"title":"  My task  ","slug":" Long / TASK! "}),
    );
    let id = created["id"].as_str().unwrap();
    assert!(id.ends_with("-long-task"));
    assert_eq!(created["title"], "My task");
    assert_eq!(created["createdAt"], created["updatedAt"]);
    assert_eq!(created["specFile"], format!(".keel/tasks/{id}/spec.md"));
    let task_dir = root.join(created["dir"].as_str().unwrap());
    let mut meta: Value =
        serde_json::from_slice(&fs::read(task_dir.join("meta.json")).unwrap()).unwrap();
    assert_eq!(meta["status"], "open");
    assert!(meta.get("dir").is_none());
    let spec = fs::read_to_string(task_dir.join("spec.md")).unwrap();
    let show = server.data("keel_task_show", json!({"id":id}));
    assert_eq!(show["spec"], spec);
    assert!(spec.contains("## Acceptance Criteria\n\n- [ ]"));
    let next = server.data(
        "keel_task_create",
        json!({"title":"New", "slug":"long-task"}),
    );
    assert_eq!(next["id"], format!("{id}-2"));
    let archive = root.join(".keel/tasks/archive");
    fs::create_dir(&archive).unwrap();
    meta["status"] = json!("done");
    put(&task_dir.join("meta.json"), meta.to_string());
    fs::rename(&task_dir, archive.join(id)).unwrap();
    assert_eq!(
        server.data("keel_task_list", json!({"status":"ignored"})),
        json!([next])
    );
    assert_eq!(
        server.data("keel_task_list", json!({"status":"done"})),
        json!([])
    );
    let archived = server.data(
        "keel_task_list",
        json!({"status":"done","includeArchived":true}),
    );
    assert_eq!(archived[0]["dir"], format!(".keel/tasks/archive/{id}"));
    assert_eq!(
        server.text("keel_task_show", json!({"id":id}), true),
        format!("Task not found: {id}")
    );
    assert_eq!(
        server.data("keel_task_show", json!({"id":format!("archive/{id}")}))["spec"],
        spec
    );
    server.finish();
}

#[test]
fn verify_captures_both_streams_honors_cwd_subset_order_and_fail_fast() {
    let fixture = Fixture::new();
    let mut server = fixture.start();
    assert_eq!(
        server.data("keel_verify", json!({})),
        json!({"passed":true,"gates":[]})
    );
    put(&fixture.root.join("work dir/marker.txt"), "from-cwd");
    let read = if cfg!(windows) {
        "type marker.txt"
    } else {
        "cat marker.txt"
    };
    let fail = if cfg!(windows) {
        "echo stdout&echo stderr 1>&2&exit /b 7"
    } else {
        "echo stdout; echo stderr >&2; exit 7"
    };
    let file = fixture.root.join(".keel/config.yaml");
    put(
        &file,
        format!(
            "verify:\n  gates:\n    - name: first\n      command: {read}\n      cwd: 'work dir'\n    - name: broken\n      command: '{fail}'\n    - name: last\n      command: echo LAST\n    - name: bad-cwd\n      cwd: nonexistent\n      command: echo should-not-run\n"
        ),
    );
    let subset = server.data("keel_verify", json!({"gates":["last",2,"first"]}));
    assert_eq!(subset["gates"][0]["output"], "from-cwd");
    assert_eq!(subset["gates"][1]["output"], "LAST");
    let report: Value =
        serde_json::from_str(&server.text("keel_verify", json!({"failFast":true}), true)).unwrap();
    assert_eq!(report["passed"], false);
    assert_eq!(report["gates"].as_array().unwrap().len(), 2);
    let broken = &report["gates"][1];
    assert_eq!(broken["exitCode"], 7);
    assert_eq!(broken["passed"], false);
    assert_eq!(broken["command"], fail);
    assert!(broken["durationMs"].is_u64());
    assert_eq!(
        broken["output"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>(),
        ["stdout", "stderr"]
    );
    let all: Value = serde_json::from_str(&server.text("keel_verify", json!({}), true)).unwrap();
    assert_eq!(all["gates"].as_array().unwrap().len(), 4);
    assert_eq!(all["gates"][3]["exitCode"], 1);
    assert_eq!(
        server.text("keel_verify", json!({"gates":["last","absent"]}), true),
        "Unknown verification gate(s): absent"
    );
    put(&file, "verify: [unclosed");
    assert!(!server.text("keel_verify", json!({}), true).is_empty());
    assert_eq!(server.request("ping", json!({}))["result"], json!({}));
    server.finish();
}

#[test]
fn memory_scans_only_valid_turns_all_sources_and_keeps_stable_top_k() {
    let fixture = Fixture::new();
    let root = &fixture.root;
    let codex = fixture.home.join(".codex/sessions/2026/11/a session.jsonl");
    let claude = fixture.home.join(".claude/projects/work/session.jsonl");
    let opencode = fixture.home.join(".local/share/opencode/chat.jsonl");
    let custom = root.join("local notes/memo.md");
    let extra = fixture.env_dir.join("extra.jsonl");
    let artifact = root.join(".keel/tasks/task/research/decision.md");
    put(
        &root.join(".keel/config.yaml"),
        "memory: {sources: ['local notes', './local notes', 'missing directory']}\n",
    );
    put(&artifact, "# Design\nalpha");
    put(&custom, "# Notes\nalpha beta beta");
    put(
        &extra,
        r#"{"role":"human","text":"Alpha beta env","created_at":"E"}"#,
    );
    put(&opencode, r#"{"role":"model","text":"beta beta"}"#);
    put(
        &claude,
        r#"{"timestamp":"C","message":{"role":"user","content":[{"text":"alpha"},"beta claude"]}}"#,
    );
    let mut bytes = b"corrupt\n{\"role\":\"user\",\"text\":\"\xff\"}\n".to_vec();
    bytes.extend_from_slice(
        br#"{"payload":{"role":"assistant","content":"alpha"}}
{"payload":{"role":"user","content":"alpha beta codex"},"timestamp":"X"}
{"payload":{"role":"assistant","content":"alpha beta second"}}
{"role":"tool","text":"do not count"}"#,
    );
    put(&codex, bytes);
    put(&root.join("local notes/unreadable.md"), [255]);
    put(
        &root.join("local notes/only-invalid.jsonl"),
        "null\n{}\n{\"role\":\"system\",\"text\":\"alpha\"}\n",
    );
    put(
        &fixture
            .home
            .join(".codex/sessions/too/deep/for/the/cli/limit/x.jsonl"),
        r#"{"role":"user","text":"wrong depth"}"#,
    );
    for (index, path) in [&artifact, &custom, &extra, &claude, &opencode, &codex]
        .into_iter()
        .enumerate()
    {
        set_mtime(path, 1_700_000_000 + index as u64);
    }
    let mut server = fixture.start();
    let sessions = server.data("keel_mem_sessions", json!({}));
    let sessions = sessions.as_array().unwrap();
    assert_eq!(sessions.len(), 7);
    assert_eq!(sessions[0]["title"], "wrong depth");
    let sessions = &sessions[1..];
    assert_eq!(sessions[0]["source"], "codex");
    assert_eq!(sessions[0]["turnCount"], 3);
    assert_eq!(sessions[0]["title"], "alpha beta codex");
    assert_eq!(sessions[0]["updatedAt"], "2023-11-14T22:13:25.000Z");
    let native = codex.to_string_lossy();
    #[cfg(windows)]
    let native = native.replace('/', "\\");
    let hash = format!("{:x}", Sha256::digest(native.as_bytes()));
    assert_eq!(
        sessions[0]["id"],
        format!("codex:a-session-{}", &hash[..10])
    );
    assert_eq!(sessions[5]["file"], ".keel/tasks/task/research/decision.md");
    assert!(
        sessions
            .iter()
            .all(|session| !session["file"].as_str().unwrap().contains('\\'))
    );
    let hits = server.data(
        "keel_mem_search",
        json!({"query":" ALPHA  beta ","limit":4.75}),
    );
    let hits = hits.as_array().unwrap();
    assert_eq!(hits.len(), 4);
    assert_eq!(hits[0]["source"], "custom");
    assert_eq!(hits[0]["score"], 11.5);
    assert_eq!(hits[1]["sessionId"], sessions[0]["id"]);
    assert_eq!(hits[1]["snippet"], "alpha beta codex");
    assert_eq!(hits[1]["timestamp"], "X");
    assert_eq!(hits[2]["snippet"], "alpha beta second");
    assert!(hits[2].get("timestamp").is_none());
    assert_eq!(hits[3]["snippet"], "alpha beta claude");
    assert_eq!(
        server.data("keel_mem_search", json!({"source":"custom","query":"env"}))[0]["timestamp"],
        "E"
    );
    assert_eq!(
        server.data("keel_mem_sessions", json!({"source":"keel","limit":1})),
        json!([sessions[5]])
    );
    assert_eq!(
        server.data("keel_mem_sessions", json!({"source":"absent"})),
        json!([])
    );
    assert_eq!(
        server.data("keel_mem_search", json!({"query":"alpha","limit":-2})),
        json!([])
    );
    server.finish();
}

#[test]
fn cli_diagnostics_do_not_write_stdout() {
    for (args, success) in [
        (vec!["--help"], true),
        (vec!["--version"], true),
        (vec!["--root"], false),
        (vec!["--other"], false),
    ] {
        let result = Command::new(BIN).args(&args).output().unwrap();
        assert_eq!(result.status.success(), success, "{args:?}");
        assert_eq!(result.stdout.is_empty(), !success);
        assert_eq!(result.stderr.is_empty(), success);
    }
}

#[test]
fn native_server_stays_responsive_after_a_gate_tree_times_out() {
    let fixture = Fixture::new();
    put(
        &fixture.root.join("watch.cjs"),
        "require('node:child_process').spawn(process.execPath,['-e','setTimeout(()=>{},5000)'],{stdio:'inherit'});console.log('ready');",
    );
    put(
        &fixture.root.join(".keel/config.yaml"),
        "verify:\n  gates:\n  - {name: watch, command: 'node watch.cjs', timeoutMs: 600}\n  - {name: after, command: 'echo ok'}",
    );
    let mut server = fixture.start();
    let report: Value =
        serde_json::from_str(&server.text("keel_verify", json!({"failFast":true}), true)).unwrap();
    let gate = &report["gates"][0];
    assert_eq!(report["gates"].as_array().unwrap().len(), 1);
    assert_eq!(gate["exitCode"], 124);
    assert_eq!(gate["timedOut"], true);
    assert_eq!(gate["output"], "ready\n[timed out after 600ms]");
    assert_eq!(server.request("ping", json!({}))["result"], json!({}));
    assert_eq!(
        server.data("keel_verify", json!({"gates":["after"]}))["gates"][0]["output"],
        "ok"
    );
    server.finish();
}

#[test]
fn incremental_verify_and_stale_rules_are_opt_in_over_stdio() {
    let fixture = Fixture::new();
    let root = &fixture.root;
    git(root, &["init", "-q"]);
    put(
        &root.join(".keel/config.yaml"),
        "verify:\n  gates:\n    - {name: api, command: echo api, paths: [api/**]}\n    - {name: ui, command: echo ui, paths: [ui/**]}\n    - {name: all, command: echo all}\n",
    );
    put(
        &root.join(".keel/rules/scoped.md"),
        "---\nid: scope\nsummary: Scoped\npaths: [api/**]\n---\n",
    );
    let mut server = fixture.start();
    let normal = server.data("keel_rules_check", json!({}));
    assert!(normal.get("warnings").is_none());
    let stale = server.data("keel_rules_check", json!({"stale":true}));
    assert_eq!(stale["valid"], true);
    assert_eq!(stale["warnings"].as_array().unwrap().len(), 1);
    let none = server.data("keel_verify", json!({"git":true}));
    assert_eq!(none["skipped"], json!(["api", "ui"]));
    put(&root.join("api/文.txt"), "change");
    assert_eq!(
        server.data("keel_rules_check", json!({"stale":true}))["warnings"],
        json!([])
    );
    let matched = server.data("keel_verify", json!({"git":true}));
    assert_eq!(matched["skipped"], json!(["ui"]));
    assert_eq!(matched["gates"][0]["name"], "api");
    assert_eq!(matched["gates"][1]["name"], "all");
    let explicit = server.data("keel_verify", json!({"git":true,"gates":["ui"]}));
    assert_eq!(explicit["gates"][0]["name"], "ui");
    assert!(explicit.get("skipped").is_none());
    server.finish();
}

#[test]
fn one_server_completes_write_read_cycles_across_repositories_without_leakage() {
    let fixture = Fixture::new();
    let other = fixture._temp.path().join("repo-b");
    fs::create_dir_all(other.join(".keel")).unwrap();
    fs::create_dir_all(fixture.root.join(".keel")).unwrap();
    let mut server = fixture.server(&fixture.home, false);
    for name in [
        "keel_verify",
        "keel_task_create",
        "keel_task_update",
        "keel_rules_add",
    ] {
        assert!(server.text(name, json!({}), true).contains("pass \"root\""));
    }
    assert!(!fixture.home.join(".keel").exists());
    server.request("initialize",json!({"workspaceFolders":[{"uri":url::Url::from_directory_path(&fixture.root).unwrap().as_str()}]}));
    server.data("keel_rules_add", json!({"id":"Rule A","summary":"only A","severity":"adr","paths":["src/**"],"body":"precise note"}));
    assert!(
        server
            .text(
                "keel_rules_add",
                json!({"id":"rule-a","summary":"old"}),
                true
            )
            .contains("exists")
    );
    assert_eq!(
        server.data(
            "keel_rules_add",
            json!({"id":"rule-a","summary":"updated","force":true})
        )["summary"],
        "updated"
    );
    assert_eq!(
        server.data("keel_rules_list", json!({"root":other})),
        json!([])
    );
    let task = server.data("keel_task_create", json!({"root":other,"title":"Unique B"}));
    let id = &task["id"];
    assert_eq!(server.data("keel_task_list", json!({})), json!([]));
    for (action, status) in [("done", "done"), ("reopen", "open"), ("archive", "done")] {
        assert_eq!(
            server.data(
                "keel_task_update",
                json!({"root":other,"id":id,"action":action})
            )["status"],
            status
        );
    }
    assert!(
        server
            .text(
                "keel_task_update",
                json!({"id":"../other","action":"done","root":other}),
                true
            )
            .contains("Task not found")
    );
    let hits = server.data(
        "keel_mem_search",
        json!({"root":other,"scope":"project","source":"keel","query":"Unique B"}),
    );
    let session = hits[0]["sessionId"].as_str().unwrap();
    let show = server.data(
        "keel_mem_show",
        json!({"root":other,"sessionId":&session[session.len()-10..]}),
    );
    assert_eq!(show["summary"]["id"], session);
    assert!(
        show["turns"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Unique B")
    );
    assert_eq!(
        server.data("keel_hotspots", json!({"root":other})),
        json!([])
    );
    server.finish();
}

#[test]
fn stdio_locked_root_rejects_siblings_and_invalid_paths_without_writes() {
    let fixture = Fixture::new();
    put(&fixture.root.join(".keel/rules/index.md"), "# Index");
    let mut server = fixture.launch(&fixture.home, true, true);
    server.request(
        "initialize",
        json!({"rootUri":url::Url::from_directory_path(&fixture.home).unwrap().as_str()}),
    );
    for root in [
        &fixture.home,
        &fixture.root.join("missing"),
        &fixture.root.join(".keel/rules/index.md"),
    ] {
        let error = server.text(
            "keel_rules_add",
            json!({"root":root,"id":"bad","summary":"outside"}),
            true,
        );
        assert!(
            error.contains("locked root") || error.contains("Invalid root directory"),
            "{error}"
        );
    }
    server.data(
        "keel_task_create",
        json!({"root":fixture.root.join(".keel"),"title":"Inside"}),
    );
    assert!(!fixture.home.join(".keel").exists());
    assert_eq!(
        server
            .data("keel_task_list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    server.finish();
}

#[test]
fn bounded_results_across_many_files_and_a_large_transcript() {
    let fixture = Fixture::new();
    for index in 0..64 {
        let path = fixture.env_dir.join(format!("s-{index:03}.jsonl"));
        let ordinary = format!("{{\"role\":\"user\",\"text\":\"needle {index}\"}}\n");
        put(&path, ordinary.repeat(128));
        set_mtime(&path, 1_700_000_000 + index);
    }
    let large = fixture.env_dir.join("large.jsonl");
    let mut file = File::create(&large).unwrap();
    for _ in 0..20_000 {
        writeln!(
            file,
            "{{\"type\":\"event\",\"payload\":{{\"kind\":\"not a turn\"}}}}"
        )
        .unwrap();
    }
    writeln!(
        file,
        "{{\"role\":\"user\",\"text\":\"needle second term\"}}"
    )
    .unwrap();
    drop(file);
    set_mtime(&large, 1_699_999_999);
    let mut server = fixture.start();
    let sessions = server.data("keel_mem_sessions", json!({"source":"custom"}));
    assert_eq!(sessions.as_array().unwrap().len(), 20);
    assert_eq!(sessions[0]["turnCount"], 128);
    let hits = server.data(
        "keel_mem_search",
        json!({"source":"custom","query":"needle second"}),
    );
    assert_eq!(hits.as_array().unwrap().len(), 15);
    assert_eq!(hits[0]["snippet"], "needle second term");
    assert_eq!(hits[0]["score"], 11);
    assert!(
        hits.as_array().unwrap()[1..]
            .iter()
            .all(|hit| hit["snippet"] == "needle 63")
    );
    server.finish();
}

#[cfg(unix)]
#[test]
fn inaccessible_directories_do_not_hide_readable_siblings() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let locked = fixture.env_dir.join("locked");
    put(&locked.join("a.md"), "inside");
    put(&fixture.env_dir.join("b.md"), "outside");
    let original = fs::metadata(&locked).unwrap().permissions();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let inaccessible = fs::read_dir(&locked).is_err();
    let mut server = fixture.start();
    let sessions = server.data("keel_mem_sessions", json!({"source":"custom"}));
    fs::set_permissions(&locked, original).unwrap();
    assert!(
        sessions
            .as_array()
            .unwrap()
            .iter()
            .any(|session| session["title"] == "outside")
    );
    if inaccessible {
        assert_eq!(sessions.as_array().unwrap().len(), 1);
    }
    server.finish();
}
