use crate::config;
use crate::paths::{js_whitespace, trim};
use crate::{git, rules};
use anyhow::{Result, bail};
use command_group::CommandGroup;
use serde::Serialize;
use std::collections::VecDeque;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GateResult {
    name: String,
    command: String,
    passed: bool,
    exit_code: i32,
    duration_ms: u128,
    output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timed_out: Option<bool>,
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

#[derive(Default)]
struct Tail {
    chars: VecDeque<char>,
    count: usize,
}

impl Tail {
    fn push(&mut self, character: char, limit: usize) {
        if limit != 0 && self.chars.len() == limit {
            self.chars.pop_front();
        }
        self.chars.push_back(character);
        self.count += 1;
    }

    fn append(&mut self, other: Self, limit: usize) {
        self.count += other.count - other.chars.len();
        for character in other.chars {
            self.push(character, limit);
        }
    }
}

#[derive(Default)]
struct Capture {
    leading: Tail,
    body: Tail,
    trailing: Tail,
}

impl Capture {
    fn accept(&mut self, text: &str, limit: usize) {
        for character in text.chars() {
            if js_whitespace(character) {
                if self.body.count == 0 {
                    &mut self.leading
                } else {
                    &mut self.trailing
                }
                .push(character, limit);
            } else {
                if self.trailing.count != 0 {
                    self.body.append(std::mem::take(&mut self.trailing), limit);
                }
                self.body.push(character, limit);
            }
        }
    }
}

fn capture(mut stream: impl Read, limit: usize) -> io::Result<Capture> {
    let mut result = Capture::default();
    let mut buffer = [0u8; 8192];
    let mut pending = 0;
    loop {
        let size = stream.read(&mut buffer[pending..])?;
        if size == 0 {
            if pending != 0 {
                result.accept("�", limit);
            }
            return Ok(result);
        }
        let end = size + pending;
        let mut offset = 0;
        while offset < end {
            match std::str::from_utf8(&buffer[offset..end]) {
                Ok(text) => {
                    result.accept(text, limit);
                    offset = end;
                }
                Err(err) => {
                    let valid = offset + err.valid_up_to();
                    result.accept(std::str::from_utf8(&buffer[offset..valid]).unwrap(), limit);
                    offset = valid;
                    let Some(length) = err.error_len() else { break };
                    result.accept("�", limit);
                    offset += length;
                }
            }
        }
        buffer.copy_within(offset..end, 0);
        pending = end - offset;
    }
}

fn execute(gate: &config::Gate, cwd: &Path) -> Result<(i32, String, bool)> {
    let limit = gate.max_output_chars;
    let mut command = shell(&gate.command);
    command
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    let child = command.group().kill_on_drop(true).spawn();
    #[cfg(not(windows))]
    let child = command.group_spawn();
    let mut group = match child {
        Ok(group) => group,
        Err(err) => {
            return Ok((
                1,
                format!(
                    "Cannot start gate `{}` in {}: {err}",
                    gate.name,
                    crate::paths::display(cwd)
                ),
                false,
            ));
        }
    };
    let out = group.inner().stdout.take().unwrap();
    let err = group.inner().stderr.take().unwrap();
    let stdout = thread::spawn(move || capture(out, limit));
    let stderr = thread::spawn(move || capture(err, limit));
    let start = Instant::now();
    let (code, timed_out) = loop {
        if let Some(status) = group.inner().try_wait()?
            && stdout.is_finished()
            && stderr.is_finished()
        {
            break (status.code().unwrap_or(1), false);
        }
        if gate.timeout_ms != 0 && start.elapsed() >= Duration::from_millis(gate.timeout_ms) {
            group.kill()?;
            group.inner().wait()?;
            break (124, true);
        }
        thread::sleep(Duration::from_millis(4));
    };
    let out = stdout
        .join()
        .map_err(|_| anyhow::anyhow!("gate stdout reader failed"))??;
    let err = stderr
        .join()
        .map_err(|_| anyhow::anyhow!("gate stderr reader failed"))??;
    let mut tail = out.body;
    if err.body.count != 0 {
        if tail.count != 0 {
            tail.append(out.trailing, limit);
            tail.push('\n', limit);
            tail.append(err.leading, limit);
        }
        tail.append(err.body, limit);
    }
    let omitted = tail.count - tail.chars.len();
    let mut output = if omitted == 0 {
        String::new()
    } else {
        format!("[truncated: omitted {omitted} chars]\n")
    };
    output.extend(tail.chars);
    if timed_out {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!("[timed out after {}ms]", gate.timeout_ms));
    }
    Ok((code, output, timed_out))
}

pub(crate) fn run(
    root: &Path,
    names: &[&str],
    fail_fast: bool,
    selection: Option<git::Changes<'_>>,
) -> Result<Report> {
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
    let changes = selection
        .filter(|_| requested.is_empty())
        .map(|mode| git::changed_files(root, mode))
        .transpose()?
        .map(|files| files.into_iter().collect::<Vec<_>>());
    let mut skipped = changes.as_ref().map(|_| Vec::new());
    let mut selected = Vec::new();
    for gate in config.gates {
        if !requested.is_empty() && !requested.contains(&gate.name.as_str()) {
            continue;
        }
        if let (Some(paths), Some(skipped)) = (&changes, &mut skipped)
            && !gate.paths.is_empty()
            && !rules::matches_paths(&gate.paths, paths)?
        {
            skipped.push(gate.name);
            continue;
        }
        selected.push(gate);
    }
    for gate in selected {
        let cwd = gate
            .cwd
            .as_ref()
            .map_or_else(|| root.to_owned(), |dir| root.join(dir));
        let start = Instant::now();
        let (exit_code, output, timed_out) = execute(&gate, &cwd)?;
        let duration_ms = start.elapsed().as_millis();
        let passed = exit_code == 0;
        results.push(GateResult {
            name: gate.name,
            command: gate.command,
            passed,
            exit_code,
            duration_ms,
            output,
            timed_out: timed_out.then_some(true),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_decodes_chunk_boundaries_and_keeps_only_the_unicode_tail() {
        let input = format!(" {}🚀文 \t", "x".repeat(8190));
        let captured = capture(input.as_bytes(), 3).unwrap();
        assert_eq!(captured.body.count, 8192);
        assert_eq!(captured.body.chars.into_iter().collect::<String>(), "x🚀文");
        assert_eq!(captured.leading.count, 1);
        assert_eq!(captured.trailing.count, 2);
    }
}
