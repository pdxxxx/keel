use crate::paths::{display, posix};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

struct Scope {
    top: PathBuf,
    prefix: String,
}

impl Scope {
    fn new(root: &Path) -> Option<Self> {
        let path = git(root, &["rev-parse", "--show-toplevel"])?;
        let top = PathBuf::from(path.trim_end_matches(['\r', '\n']));
        let root = root.canonicalize().ok()?;
        let relative = display(root.strip_prefix(top.canonicalize().ok()?).ok()?);
        let prefix = if relative.is_empty() {
            relative
        } else {
            format!("{relative}/")
        };
        Some(Self { top, prefix })
    }
    fn local(&self, name: &str) -> Option<String> {
        name.strip_prefix(&self.prefix)
            .filter(|name| !name.is_empty())
            .map(posix)
    }
}

pub(crate) enum Changes<'a> {
    Working,
    Staged,
    Base(&'a str),
}

pub(crate) fn changed_files(
    root: &Path,
    selection: Changes<'_>,
) -> anyhow::Result<BTreeSet<String>> {
    let mut paths = BTreeSet::new();
    let Some(scope) = Scope::new(root) else {
        anyhow::bail!("Cannot locate Git worktree in {}", display(root));
    };
    let mut collect = |args: &[&str]| -> anyhow::Result<()> {
        let output = git(&scope.top, args)
            .ok_or_else(|| anyhow::anyhow!("git {} failed", args.join(" ")))?;
        paths.extend(output.split('\0').filter_map(|file| scope.local(file)));
        Ok(())
    };
    match selection {
        Changes::Working => {
            collect(&["diff", "--name-only", "-z"])?;
            collect(&["diff", "--cached", "--name-only", "-z"])?;
            collect(&["ls-files", "--others", "--exclude-standard", "-z"])?;
        }
        Changes::Staged => collect(&["diff", "--cached", "--name-only", "-z"])?,
        Changes::Base(base) => {
            collect(&[
                "diff",
                "--name-only",
                "-z",
                "--end-of-options",
                &format!("{base}...HEAD"),
                "--",
            ])?;
            collect(&["diff", "--name-only", "-z"])?;
        }
    }
    Ok(paths)
}

#[derive(Serialize)]
struct Fix {
    hash: String,
    subject: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Hotspot {
    file: String,
    score: usize,
    touches: usize,
    fix_touches: usize,
    recent_fixes: Vec<Fix>,
}

pub(crate) fn hotspots(root: &Path, commits: usize, limit: usize) -> Vec<Hotspot> {
    let Some(scope) = Scope::new(root) else {
        return vec![];
    };
    let Some(log) = git(
        &scope.top,
        &[
            "log",
            &format!("-n{commits}"),
            "--no-color",
            "--no-decorate",
            "--no-notes",
            "--format=format:%x00%H%x00%s%x00",
            "--name-only",
            "-z",
            "--",
        ],
    ) else {
        return vec![];
    };
    let mut fields = log.split('\0').peekable();
    let mut files = HashMap::new();
    while let Some(hash) = fields.find(|field| !field.is_empty()) {
        let Some(subject) = fields.next() else { break };
        let fix = subject
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|word| {
                matches!(
                    word.to_ascii_lowercase().as_str(),
                    "fix" | "bug" | "revert" | "hotfix" | "patch"
                )
            });
        let mut touched = BTreeSet::new();
        let mut first = true;
        while let Some(field) = fields.next_if(|field| !field.is_empty()) {
            let file = if first {
                field.strip_prefix('\n').unwrap_or(field)
            } else {
                field
            };
            first = false;
            if let Some(local) = scope.local(file) {
                touched.insert(local);
            }
        }
        for file in touched {
            let item = files.entry(file.clone()).or_insert_with(|| Hotspot {
                file,
                score: 0,
                touches: 0,
                fix_touches: 0,
                recent_fixes: vec![],
            });
            item.touches += 1;
            if fix {
                item.fix_touches += 1;
                if item.recent_fixes.len() < 3 {
                    item.recent_fixes.push(Fix {
                        hash: hash.chars().take(8).collect(),
                        subject: subject.to_owned(),
                    });
                }
            }
            item.score = item.fix_touches * 3 + item.touches;
        }
    }
    let mut hits: Vec<_> = files
        .into_values()
        .filter(|item| root.join(&item.file).is_file())
        .collect();
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.file.encode_utf16().cmp(right.file.encode_utf16()))
    });
    hits.truncate(limit);
    hits
}
