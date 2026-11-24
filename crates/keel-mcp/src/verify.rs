use crate::config;
use crate::paths::trim;
use crate::{git, rules};
use anyhow::{Result, bail};
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GateResult {
    name: String,
    command: String,
    passed: bool,
    exit_code: i32,
    duration_ms: u128,
    output: String,
}

#[derive(Serialize)]
pub(crate) struct Report {
    pub passed: bool,
    gates: Vec<GateResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<Vec<String>>,
}

#[cfg(windows)]
fn shell(script: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("cmd.exe");
    command
        .args(["/D", "/S", "/C"])
        .raw_arg(format!("\"{script}\""));
    command
}

#[cfg(not(windows))]
fn shell(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", script]);
    command
}

pub(crate) fn run(root: &Path, names: &[&str], fail_fast: bool, git: bool) -> Result<Report> {
    let config = config::load(root)?;
    let requested: Vec<_> = names
        .iter()
        .copied()
        .filter(|name| !trim(name).is_empty())
        .collect();
    let missing: Vec<_> = requested
        .iter()
        .copied()
        .filter(|name| !config.gates.iter().any(|gate| gate.name == *name))
        .collect();
    if !missing.is_empty() {
        bail!("Unknown verification gate(s): {}", missing.join(", "));
    }

    let mut results = Vec::new();
    let changes = (git && requested.is_empty())
        .then(|| git::changed_files(root).into_iter().collect::<Vec<_>>());
    let mut skipped = changes.as_ref().map(|_| Vec::new());
    let mut selected = Vec::new();
    for gate in config.gates {
        if !requested.is_empty() && !requested.contains(&gate.name.as_str()) {
            continue;
        }
        if let (Some(paths), Some(skipped)) = (&changes, &mut skipped) {
            if !gate.paths.is_empty() && !rules::matches_paths(&gate.paths, paths)? {
                skipped.push(gate.name);
                continue;
            }
        }
        selected.push(gate);
    }
    for gate in selected {
        let cwd = gate
            .cwd
            .as_ref()
            .map_or_else(|| root.to_owned(), |dir| root.join(dir));
        let start = Instant::now();
        let proc = shell(&gate.command)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .output();
        let duration_ms = start.elapsed().as_millis();
        let (exit_code, output) = match proc {
            Ok(proc) => {
                let output = [&proc.stdout, &proc.stderr]
                    .into_iter()
                    .filter(|bytes| !bytes.is_empty())
                    .map(|bytes| String::from_utf8_lossy(bytes))
                    .collect::<Vec<_>>()
                    .join("\n");
                (proc.status.code().unwrap_or(1), trim(&output).to_owned())
            }
            Err(_) => (1, String::new()),
        };
        let passed = exit_code == 0;
        results.push(GateResult {
            name: gate.name,
            command: gate.command,
            passed,
            exit_code,
            duration_ms,
            output,
        });
        if !passed && fail_fast {
            break;
        }
    }
    Ok(Report {
        passed: results.iter().all(|gate| gate.passed),
        gates: results,
        skipped,
    })
}
