use crate::{config, init, rules};
use serde::Serialize;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

#[derive(Default, Serialize)]
pub(crate) struct Health {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub(crate) fn check(root: &Path) -> Health {
    let mut report = Health::default();
    for dir in [".keel", ".keel/rules", ".keel/tasks"] {
        if !root.join(dir).is_dir() {
            report.errors.push(format!("Missing directory: {dir}"));
        }
    }
    if !root.join(".keel/rules/index.md").is_file() {
        report
            .errors
            .push("Missing file: .keel/rules/index.md".into());
    }
    match config::inspect(root) {
        Ok((_, errors)) => report.errors.extend(errors),
        Err(err) => report.errors.push(format!(".keel/config.yaml: {err}")),
    }
    match fs::read_to_string(root.join("AGENTS.md"))
        .map_err(anyhow::Error::from)
        .and_then(|text| init::agents_range(&text))
    {
        Ok(None) => report
            .errors
            .push("AGENTS.md: missing Keel managed block".into()),
        Err(err) => report.errors.push(format!("AGENTS.md: {err}")),
        _ => (),
    }
    if !Command::new("git")
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .is_ok_and(|out| out.status.success())
    {
        report.errors.push("git is unavailable on PATH".into());
    }
    match rules::check(root, true) {
        Ok(check) => {
            let message = |issue: rules::Issue| format!("{}: {}", issue.file, issue.message);
            report.errors.extend(check.issues.into_iter().map(message));
            report
                .warnings
                .extend(check.warnings.into_iter().flatten().map(message));
        }
        Err(err) => report.errors.push(format!(".keel/rules: {err}")),
    }
    report
}
