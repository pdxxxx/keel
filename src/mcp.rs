pub use crate::mcp_root::McpRoots;
use crate::paths::trim;
use crate::{git, mem, rules, tasks, verify};
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{self, BufRead, Write};
use std::path::Path;

const PROTOCOL_VERSION: &str = "2024-11-05";

fn tool(name: &str, description: &str, mut properties: Value, required: &[&str]) -> Value {
    properties["root"] = json!({"type": "string", "description": "Absolute path to the target repository (or any directory inside it). Always pass this whenever known so a shared or aggregated MCP server targets the right repository."});
    let mut schema =
        json!({"type": "object", "properties": properties, "additionalProperties": false});
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    json!({"name": name, "description": description, "inputSchema": schema})
}

fn tools() -> Value {
    let scope = json!({"type":"string", "enum":["project","all"], "default":"all"});
    json!({"tools": [
        tool("keel_rules_list", "List all engineering rules, invariants, and ADRs in .keel/rules/.", json!({}), &[]),
        tool("keel_rules_match", "Match scoped engineering invariants for target repository paths or current git changes.", json!({
            "paths": {"type": "array", "items": {"type": "string"}, "description": "Repository-relative file paths to match."},
            "git": {"type": "boolean", "description": "If true, include current git changed files."},
            "includeGlobal": {"type": "boolean", "description": "Include global rules with no path filter (default true)."}
        }), &[]),
        tool("keel_rules_check", "Validate frontmatter, IDs, and severities of all .keel/rules/ files.", json!({
            "stale": {"type":"boolean", "description":"Check unmatched paths and rule hygiene."}
        }), &[]),
        tool("keel_task_list", "List lightweight task specs under .keel/tasks/.", json!({
            "status": {"type": "string", "enum": ["open", "done"]}, "includeArchived": {"type": "boolean"}
        }), &[]),
        tool("keel_task_create", "Create a lightweight task directory with spec.md and meta.json.", json!({
            "title": {"type": "string"}, "slug": {"type": "string"}
        }), &["title"]),
        tool("keel_task_show", "Read a task's metadata and living spec.md contents.", json!({"id": {"type": "string"}}), &["id"]),
        tool("keel_verify", "Run configured deterministic verification gates from .keel/config.yaml.", json!({
            "gates": {"type": "array", "items": {"type": "string"}, "description": "Optional subset of gate names to run."},
            "failFast": {"type": "boolean"},
            "git": {"type":"boolean", "description":"Filter by uncommitted changes unless gate names are supplied."}
        }), &[]),
        tool("keel_mem_search", "Search across local AI coding transcripts and repository Keel task/rule artifacts.", json!({
            "query": {"type": "string"}, "source": {"type": "string"}, "limit": {"type": "number"}, "scope": scope
        }), &["query"]),
        tool("keel_mem_sessions", "List recent local AI coding sessions and repository artifacts.", json!({
            "source": {"type": "string"}, "limit": {"type": "number"}, "scope":scope
        }), &[]),
        tool("keel_mem_show", "Read a memory session by full ID or unique suffix.", json!({"sessionId":{"type":"string"},"limit":{"type":"integer","minimum":1,"default":30}}), &["sessionId"]),
        tool("keel_rules_add", "Create or replace a scoped engineering rule.", json!({"id":{"type":"string"},"summary":{"type":"string"},"severity":{"type":"string","enum":["invariant","guideline","adr"]},"paths":{"type":"array","items":{"type":"string"}},"body":{"type":"string"},"force":{"type":"boolean"}}), &["id","summary"]),
        tool("keel_task_update", "Mark a task done, reopen it, or archive it.", json!({"id":{"type":"string"},"action":{"type":"string","enum":["done","reopen","archive"]}}), &["id","action"]),
        tool("keel_hotspots", "Rank existing project files by recent changes and bugfix commits.", json!({"commits":{"type":"integer","minimum":1,"default":100},"limit":{"type":"integer","minimum":1,"default":10}}), &[])
    ]})
}

fn text(message: String, is_error: bool) -> Value {
    let mut result = json!({"content": [{"type": "text", "text": message}]});
    if is_error {
        result["isError"] = Value::Bool(true);
    }
    result
}

fn payload(value: impl Serialize, is_error: bool) -> Result<Value> {
    Ok(text(serde_json::to_string_pretty(&value)?, is_error))
}

fn strings<'args>(args: &'args Value, field: &str) -> Vec<&'args str> {
    args[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn required<'args>(args: &'args Value, field: &str) -> Result<&'args str> {
    args[field]
        .as_str()
        .filter(|value| !trim(value).is_empty())
        .ok_or_else(|| anyhow!("Missing required string argument: {field}"))
}

fn limit(args: &Value, default: usize) -> usize {
    args["limit"]
        .as_f64()
        .map(|value| value as usize)
        .unwrap_or(default)
}

fn call(root: &Path, name: &str, args: &Value) -> Result<Value> {
    let project = || match args.get("scope") {
        None => Ok(false),
        Some(value) if value == "all" => Ok(false),
        Some(value) if value == "project" => Ok(true),
        _ => Err(anyhow!("Invalid scope: expected project or all")),
    };
    let positive = |name: &str, default| {
        args[name]
            .as_f64()
            .filter(|n| n.fract() == 0.0 && (1.0..=9_007_199_254_740_991.0).contains(n))
            .map_or(default, |n| n as usize)
    };
    match name {
        "keel_rules_list" => payload(rules::list(root)?, false),
        "keel_rules_match" => {
            let mut targets: BTreeSet<String> = strings(args, "paths")
                .into_iter()
                .map(str::to_owned)
                .collect();
            if args["git"] == true {
                targets.extend(git::changed_files(root, git::Changes::Working)?);
            }
            payload(
                rules::matching(
                    root,
                    &targets.into_iter().collect::<Vec<_>>(),
                    args["includeGlobal"] != false,
                )?,
                false,
            )
        }
        "keel_rules_check" => {
            let report = rules::check(root, args["stale"] == true)?;
            payload(&report, !report.valid)
        }
        "keel_task_list" => {
            let status = args["status"]
                .as_str()
                .filter(|status| matches!(*status, "open" | "done"));
            payload(
                tasks::list(root, status, args["includeArchived"] == true)?,
                false,
            )
        }
        "keel_task_create" => payload(
            tasks::create(root, required(args, "title")?, args["slug"].as_str())?,
            false,
        ),
        "keel_task_show" => payload(tasks::show(root, required(args, "id")?)?, false),
        "keel_verify" => {
            let report = verify::run(
                root,
                &strings(args, "gates"),
                args["failFast"] == true,
                (args["git"] == true).then_some(git::Changes::Working),
            )?;
            payload(&report, !report.passed)
        }
        "keel_mem_search" => payload(
            mem::search(
                root,
                required(args, "query")?,
                args["source"].as_str(),
                limit(args, 15),
                project()?,
            )?,
            false,
        ),
        "keel_mem_sessions" => payload(
            mem::sessions(root, args["source"].as_str(), limit(args, 20), project()?)?,
            false,
        ),
        "keel_mem_show" => payload(
            mem::show(root, required(args, "sessionId")?, positive("limit", 30))?,
            false,
        ),
        "keel_rules_add" => {
            let id = required(args, "id")?;
            let summary = required(args, "summary")?;
            if args.get("severity").is_some_and(|value| {
                !matches!(value.as_str(), Some("invariant" | "guideline" | "adr"))
            }) {
                bail!("Invalid severity: expected invariant, guideline or adr")
            }
            payload(
                rules::add(
                    root,
                    id,
                    summary,
                    args["severity"].as_str(),
                    &strings(args, "paths"),
                    args["body"].as_str(),
                    args["force"] == true,
                )?,
                false,
            )
        }
        "keel_task_update" => payload(
            tasks::update(root, required(args, "id")?, required(args, "action")?)?,
            false,
        ),
        "keel_hotspots" => payload(
            git::hotspots(root, positive("commits", 100), positive("limit", 10)),
            false,
        ),
        _ => bail!("Unknown tool: {name}"),
    }
}

fn error(id: &Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn handle_request(roots: &mut McpRoots, request: &Value) -> Option<Value> {
    let id = &request["id"];
    let Some(method) = request["method"].as_str() else {
        return Some(error(&Value::Null, -32600, "Invalid Request"));
    };
    if method.starts_with("notifications/") {
        return None;
    }
    if request
        .get("jsonrpc")
        .is_some_and(|version| version.as_str() != Some("2.0"))
    {
        return Some(error(&Value::Null, -32600, "Invalid Request"));
    }
    if !(id.is_null() || id.is_number() || id.is_string()) {
        return Some(error(&Value::Null, -32600, "Invalid Request"));
    }
    let result = match method {
        "initialize" => {
            roots.initialize(&request["params"]);
            json!({"protocolVersion": PROTOCOL_VERSION, "capabilities": {"tools": {}},
                "serverInfo": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")}})
        }
        "ping" => json!({}),
        "tools/list" => tools(),
        "tools/call" => {
            let params = &request["params"];
            let Some(name) = params["name"].as_str().filter(|name| !name.is_empty()) else {
                return Some(error(id, -32602, "Missing tool name"));
            };
            let args = &params["arguments"];
            if args.get("root").is_some_and(|raw| !raw.is_string()) {
                text(format!("Invalid root directory: {}", args["root"]), true)
            } else {
                roots
                    .for_tool(name, args["root"].as_str())
                    .and_then(|root| call(&root, name, args))
                    .unwrap_or_else(|err| text(err.to_string(), true))
            }
        }
        _ => return Some(error(id, -32601, &format!("Method not found: {method}"))),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

pub fn serve_session(
    mut roots: McpRoots,
    mut input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    let mut line = Vec::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        let frame = std::str::from_utf8(&line).ok().map(trim);
        if frame == Some("") {
            continue;
        }
        let response = match frame.and_then(|text| serde_json::from_str(text).ok()) {
            Some(request) => handle_request(&mut roots, &request),
            None => Some(error(&Value::Null, -32700, "Parse error")),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_frames_do_not_interrupt_following_requests() {
        let input = b" \r\n\xff\nnull\n[]\n{}\n{\"method\":\"notifications/test\"}\n{\"id\":\"ok\",\"method\":\"ping\"}";
        let mut output = Vec::new();
        let roots = McpRoots::new(Path::new("."), None, false).unwrap();
        serve_session(roots, &input[..], &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        let frames: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(frames.len(), 5);
        assert_eq!(frames[0], error(&Value::Null, -32700, "Parse error"));
        assert!(
            frames[1..4]
                .iter()
                .all(|frame| frame["error"]["code"] == -32600)
        );
        assert_eq!(frames[4], json!({"jsonrpc":"2.0", "id":"ok", "result":{}}));
    }
}
