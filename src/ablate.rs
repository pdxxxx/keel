use crate::{
    init,
    snapshot::{self, Operation, Plan, safe_path},
};
use anyhow::Result;
use serde_json::Value;
use std::{fs, path::Path};

pub(crate) fn run(root: &Path, state: Option<&Path>, dry_run: bool) -> Result<Value> {
    let mut plan = Plan::default();
    plan.tree(root, ".keel")?;
    let skills = safe_path(root, ".agents/skills")?;
    if skills.try_exists()? {
        for dir in fs::read_dir(skills)? {
            let dir = dir?;
            let name = dir.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("keel-") {
                let rel = format!(".agents/skills/{name}");
                if safe_path(root, &rel)?.is_dir() {
                    plan.tree(root, &rel)?;
                }
            }
        }
    }
    let path = safe_path(root, "AGENTS.md")?;
    if path.try_exists()? {
        let text = fs::read_to_string(path)?;
        if let Some(body) = init::strip(&text, "KEEL", "AGENTS.md")? {
            plan.edit(
                "AGENTS.md",
                text.as_bytes(),
                (!body.is_empty()).then(|| body.into_bytes()),
            );
        }
    }
    snapshot::apply(root, Operation::Ablate, plan, state, dry_run)
}
