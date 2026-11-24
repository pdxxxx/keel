use crate::paths::{check_write, write_bytes};
use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::HashSet, fs, ops::Range, path::Path};

const BLOCK: &str = "<!-- KEEL:START -->\n# Keel Engineering Context\n\nUse `keel rules match <paths...>` before non-trivial edits. Record multi-step contracts with `keel task create` and retrieve past decisions with `keel mem search`.\nWhen calling `keel_*` MCP tools, pass the target repository path as `root` whenever known.\nRun `keel verify` before finishing. Prefer executable gates over prose; keep rules scoped to non-obvious invariants. Keel does not prescribe how your agent schedules work.\n<!-- KEEL:END -->";
const INDEX: &str = include_str!("../.keel/rules/index.md");
const SKILLS: &[(&str, &str)] = &[
    (
        "keel-plan",
        include_str!("../.agents/skills/keel-plan/SKILL.md"),
    ),
    (
        "keel-verify",
        include_str!("../.agents/skills/keel-verify/SKILL.md"),
    ),
    (
        "keel-distill",
        include_str!("../.agents/skills/keel-distill/SKILL.md"),
    ),
    (
        "keel-mem",
        include_str!("../.agents/skills/keel-mem/SKILL.md"),
    ),
    (
        "keel-migrate",
        include_str!("../.agents/skills/keel-migrate/SKILL.md"),
    ),
];

pub(crate) fn blocks(text: &str, label: &str, file: &str) -> Result<Vec<Range<usize>>> {
    let prefix = Regex::new(&format!(r"(?i-u)<!--\s*{}\s*:", regex::escape(label)))?;
    let start = format!("<!-- {label}:START -->");
    let end = format!("<!-- {label}:END -->");
    let mut open = None;
    let mut ranges = Vec::new();
    for token in prefix.find_iter(text) {
        let at = token.start();
        if text[at..].starts_with(&start)
            && open.is_none()
            && (label != "KEEL" || ranges.is_empty())
        {
            open = Some(at);
        } else if text[at..].starts_with(&end)
            && let Some(from) = open.take()
        {
            ranges.push(from..at + end.len());
        } else {
            bail!("Malformed {label} markers in {file}");
        }
    }
    ensure!(open.is_none(), "Unpaired {label} markers in {file}");
    Ok(ranges)
}

pub(crate) fn strip(text: &str, label: &str, file: &str) -> Result<Option<String>> {
    let ranges = blocks(text, label, file)?;
    if ranges.is_empty() {
        return Ok(None);
    }
    check_overlap(text, label, file, &ranges)?;
    let mut result = text.to_owned();
    for range in ranges.into_iter().rev() {
        let after = &result[range.end..];
        let newline = if after.starts_with("\r\n") {
            2
        } else {
            usize::from(after.starts_with('\n'))
        };
        result.replace_range(range.start..range.end + newline, "");
    }
    Ok(Some(result))
}

fn check_overlap(text: &str, label: &str, file: &str, ranges: &[Range<usize>]) -> Result<()> {
    let other = if label == "KEEL" { "TRELLIS" } else { "KEEL" };
    for between in blocks(text, other, file)? {
        ensure!(
            !ranges
                .iter()
                .any(|range| range.start < between.end && between.start < range.end),
            "Overlapping {label} and {other} markers in {file}"
        );
    }
    Ok(())
}

pub(crate) fn agents_range(text: &str) -> Result<Option<Range<usize>>> {
    let mut ranges = blocks(text, "KEEL", "AGENTS.md")?;
    check_overlap(text, "KEEL", "AGENTS.md", &ranges)?;
    Ok(ranges.pop())
}

fn agents(text: &str) -> Result<String> {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let block = BLOCK.replace('\n', newline);
    let mut result = text.to_owned();
    if let Some(range) = agents_range(text)? {
        result.replace_range(range, &block);
    } else {
        if !text.trim_start_matches('\u{feff}').is_empty() {
            if !text.ends_with('\n') {
                result.push_str(newline);
            }
            result.push_str(newline);
        }
        result.push_str(&block);
        result.push_str(newline);
    }
    Ok(result)
}

#[derive(Serialize)]
pub(crate) struct SuggestedGate {
    name: String,
    command: String,
}

pub(crate) fn detect(root: &Path) -> Result<Vec<SuggestedGate>> {
    let mut gates = Vec::new();
    let mut names = HashSet::new();
    let mut add = |platform: &str, name: &str, command: String| {
        let name = if names.insert(name.to_owned()) {
            name.to_owned()
        } else {
            format!("{platform}-{name}")
        };
        gates.push(SuggestedGate { name, command });
    };
    if root.join("package.json").is_file() {
        let data: Value = serde_json::from_slice(&fs::read(root.join("package.json"))?)
            .context("Invalid package.json")?;
        let runner = [
            ("pnpm-lock.yaml", "pnpm"),
            ("yarn.lock", "yarn"),
            ("bun.lock", "bun run"),
            ("bun.lockb", "bun run"),
        ]
        .into_iter()
        .find(|(lock, _)| root.join(lock).is_file())
        .map_or("npm run", |(_, cmd)| cmd);
        for name in ["typecheck", "lint", "build", "test"] {
            if data["scripts"][name]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty())
            {
                add("node", name, format!("{runner} {name}"));
            }
        }
    }
    for (manifest, platform, commands) in [
        (
            "Cargo.toml",
            "cargo",
            &[("check", "cargo check"), ("test", "cargo test")][..],
        ),
        (
            "go.mod",
            "go",
            &[("vet", "go vet ./..."), ("test", "go test ./...")],
        ),
        ("pyproject.toml", "python", &[("test", "pytest")]),
    ] {
        if root.join(manifest).is_file() {
            for &(name, command) in commands {
                add(platform, name, command.to_owned());
            }
        }
    }
    Ok(gates)
}

pub(crate) enum Setup {
    Init {
        force: bool,
        reset: bool,
        detect: bool,
    },
    Update {
        dry_run: bool,
    },
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    dry_run: bool,
    created: Vec<String>,
    updated: Vec<String>,
    skipped: Vec<String>,
    detected_gates: Vec<SuggestedGate>,
}

pub(crate) fn setup(root: &Path, mode: Setup, no_skills: bool) -> Result<Report> {
    let (force, dry_run) = match mode {
        Setup::Init { force, .. } => (force, false),
        Setup::Update { dry_run } => {
            ensure!(
                root.join(".keel/config.yaml").is_file(),
                "Repository is not initialized; run `keel init` first"
            );
            (true, dry_run)
        }
    };
    let mut report = Report {
        dry_run,
        ..Default::default()
    };
    let mut targets = Vec::new();
    let path = root.join("AGENTS.md");
    check_write(root, &path)?;
    let text = if path.try_exists()? {
        fs::read_to_string(&path).context("Cannot read AGENTS.md")?
    } else {
        String::new()
    };
    targets.push(("AGENTS.md".to_owned(), agents(&text)?, true));
    if let Setup::Init {
        reset,
        detect: auto,
        ..
    } = mode
    {
        if auto && (reset || !root.join(".keel/config.yaml").try_exists()?) {
            report.detected_gates = detect(root)?;
        }
        let config =
            json!({"version":1,"verify":{"gates":report.detected_gates},"memory":{"sources":[]}});
        targets.push((
            ".keel/config.yaml".into(),
            yaml_serde::to_string(&config)?,
            reset,
        ));
        targets.push((".keel/tasks/.gitkeep".into(), String::new(), false));
    }
    targets.push((".keel/rules/index.md".into(), INDEX.into(), force));
    if !no_skills {
        for (name, skill) in SKILLS {
            targets.push((
                format!(".agents/skills/{name}/SKILL.md"),
                (*skill).to_owned(),
                force,
            ));
        }
    }
    let mut writes = Vec::new();
    for (file, content, overwrite) in targets {
        let path = root.join(&file);
        check_write(root, &path)?;
        let exists = path.try_exists()?;
        ensure!(!exists || path.is_file(), "Expected a regular file: {file}");
        if exists && (!overwrite || fs::read(&path)? == content.as_bytes()) {
            report.skipped.push(file);
            continue;
        }
        if exists {
            report.updated.push(file);
        } else {
            report.created.push(file);
        }
        writes.push((path, content, exists));
    }
    if !dry_run {
        for (path, content, replace) in writes {
            write_bytes(&path, content.as_bytes(), replace)?;
        }
    }
    Ok(report)
}
