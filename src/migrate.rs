use crate::{
    init,
    paths::relative,
    rules,
    snapshot::{self, Operation, Plan, safe_path},
};
use anyhow::{Context, Result, ensure};
use regex::RegexSet;
use serde::Serialize;
use serde_json::{Value, json};
use std::{fs, path::Path};
use walkdir::WalkDir;

const SETTINGS: [&str; 4] = [
    ".claude/settings.json",
    ".codex/hooks.json",
    ".cursor/hooks.json",
    ".gemini/settings.json",
];
const HOOKS: [&str; 3] = [".claude/hooks", ".codex/hooks", ".cursor/hooks"];

#[derive(Serialize)]
struct Item {
    framework: &'static str,
    category: &'static str,
    path: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    globs: Vec<String>,
}

#[derive(Default, Serialize)]
pub(crate) struct Inventory {
    items: Vec<Item>,
}

impl Inventory {
    fn add(&mut self, framework: &'static str, category: &'static str, file: &str) {
        self.items.push(Item {
            framework,
            category,
            path: file.into(),
            globs: Vec::new(),
        });
    }
}

fn files(root: &Path, dir: &str, depth: usize) -> Result<Vec<String>> {
    let dir = safe_path(root, dir)?;
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(dir)
        .min_depth(1)
        .max_depth(depth)
        .sort_by_file_name()
    {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(relative(root, entry.path()));
        }
    }
    Ok(files)
}

fn globs(text: &str) -> Vec<String> {
    let Some((Ok(data), _)) = rules::frontmatter(text) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    if let Some(list) = data["globs"].as_sequence() {
        values.extend(list.iter().filter_map(|v| v.as_str()));
    }
    if let Some(text) = data["globs"].as_str() {
        let mut depth = 0_usize;
        values.extend(text.split(|ch| match ch {
            '{' => {
                depth += 1;
                false
            }
            '}' => {
                depth = depth.saturating_sub(1);
                false
            }
            ',' => depth == 0,
            _ => false,
        }));
    }
    values
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(crate::paths::posix)
        .collect()
}

fn references(settings: &str, hooks: &[String]) -> Result<RegexSet> {
    let mut patterns = vec![r"(?i-u)trellis".into()];
    let parent = settings
        .split_once('/')
        .map(|(name, _)| format!("{name}/"))
        .unwrap_or_default();
    for hook in hooks {
        for name in std::iter::once(hook.as_str()).chain(hook.strip_prefix(&parent)) {
            let prefix = if name.starts_with("hooks/") {
                r#"(?:^|[\s"'(=])(?:\./)?"#
            } else {
                r#"(?:^|[\s/"'(=])"#
            };
            patterns.push(format!(
                r#"(?-u){prefix}{}(?:$|[\s"';&|)])"#,
                regex::escape(name)
            ));
        }
    }
    Ok(RegexSet::new(patterns)?)
}

fn retired(value: &Value, names: &RegexSet) -> bool {
    fn contains(value: &Value, names: &RegexSet) -> bool {
        match value {
            Value::String(text) => names.is_match(&text.replace('\\', "/")),
            Value::Array(items) => items.iter().any(|v| contains(v, names)),
            Value::Object(object) => object.values().any(|v| contains(v, names)),
            _ => false,
        }
    }
    if value.is_string() {
        return contains(value, names);
    }
    ["command", "args", "script", "path", "executable"]
        .iter()
        .any(|key| contains(&value[key], names))
}

fn clean_settings(data: &mut Value, names: &RegexSet) -> bool {
    let Some(hooks) = data.get_mut("hooks").and_then(Value::as_object_mut) else {
        return false;
    };
    let mut changed = false;
    hooks.retain(|_, groups| {
        let Some(groups) = groups.as_array_mut() else {
            return true;
        };
        let original = groups.len();
        groups.retain_mut(|group| {
            if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                let before = list.len();
                list.retain(|hook| !retired(hook, names));
                changed |= before != list.len();
                return before == 0 || !list.is_empty();
            }
            !retired(group, names)
        });
        changed |= groups.len() != original;
        original == 0 || !groups.is_empty()
    });
    if changed
        && hooks.is_empty()
        && let Some(object) = data.as_object_mut()
    {
        object.remove("hooks");
    }
    changed
}

pub(crate) fn scan(root: &Path) -> Result<Inventory> {
    let mut inventory = Inventory::default();
    if safe_path(root, ".trellis")?.is_dir() {
        inventory.add("trellis", "conflict", ".trellis");
    }
    let marker = regex::Regex::new(r"(?i-u)<!--\s*TRELLIS\s*:")?;
    for file in ["AGENTS.md", "CLAUDE.md"] {
        let path = safe_path(root, file)?;
        if path.is_file() && marker.is_match(&fs::read_to_string(path)?) {
            inventory.add("trellis", "conflict", file);
        }
    }
    for dir in [
        ".claude/agents",
        ".cursor/agents",
        ".cursor/commands",
        ".agents/skills",
        ".codex/agents",
    ] {
        let path = safe_path(root, dir)?;
        if !path.is_dir() {
            continue;
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let prefix = if dir.ends_with("commands") {
                "trellis"
            } else {
                "trellis-"
            };
            if !name.to_ascii_lowercase().starts_with(prefix) {
                continue;
            }
            let file = format!("{dir}/{name}");
            let path = safe_path(root, &file)?;
            if path.is_dir() || (dir != ".agents/skills" && path.is_file()) {
                inventory.add("trellis", "conflict", &file);
            }
        }
    }
    let mut hooks = Vec::new();
    for dir in HOOKS {
        for file in files(root, dir, usize::MAX)? {
            let bytes = fs::read(safe_path(root, &file)?)?;
            if file.to_ascii_lowercase().contains("trellis")
                || bytes
                    .windows(7)
                    .any(|window| window.eq_ignore_ascii_case(b"trellis"))
            {
                inventory.add("trellis", "conflict", &file);
                hooks.push(file);
            }
        }
    }
    for file in SETTINGS {
        let path = safe_path(root, file)?;
        if !path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(path)?;
        let names = references(file, &hooks)?;
        match serde_json::from_str(&raw) {
            Ok(mut data) => {
                if clean_settings(&mut data, &names) {
                    inventory.add("trellis", "conflict", file);
                } else if names.is_match(&raw) {
                    inventory.add("trellis", "rule-source", file);
                }
            }
            Err(_) if names.is_match(&raw) => inventory.add("trellis", "conflict", file),
            Err(_) => (),
        }
    }
    if safe_path(root, ".cursorrules")?.is_file() {
        inventory.add("cursor", "conflict", ".cursorrules");
    }
    for file in files(root, ".cursor/rules", 1)?
        .into_iter()
        .filter(|s| s.ends_with(".mdc"))
    {
        let globs = globs(&fs::read_to_string(safe_path(root, &file)?)?);
        inventory.items.push(Item {
            framework: "cursor",
            category: "rule-source",
            path: file,
            globs,
        });
    }
    for (framework, file) in [
        ("claude", "CLAUDE.md"),
        ("copilot", ".github/copilot-instructions.md"),
        ("windsurf", ".windsurfrules"),
    ] {
        if safe_path(root, file)?.is_file() {
            inventory.add(framework, "rule-source", file);
        }
    }
    for (framework, dir) in [
        ("trellis", ".trellis/spec"),
        ("windsurf", ".windsurf/rules"),
        ("spec-kit", ".specify"),
        ("bmad", "_bmad"),
    ] {
        if safe_path(root, dir)?.is_dir() {
            inventory.add(framework, "rule-source", dir);
            for file in files(root, dir, usize::MAX)? {
                if framework == "windsurf" || file.ends_with(".md") || file.ends_with(".mdc") {
                    inventory.add(framework, "rule-source", &file);
                }
            }
        }
    }
    for (framework, dir) in [
        ("project", "docs/adr"),
        ("trellis", ".trellis/tasks/archive"),
    ] {
        if safe_path(root, dir)?.is_dir() {
            inventory.add(framework, "memory-source", dir);
        }
    }
    for file in [
        "package.json",
        "Cargo.toml",
        "go.mod",
        "pyproject.toml",
        "Makefile",
    ]
    .map(str::to_owned)
    .into_iter()
    .chain(
        files(root, ".github/workflows", 1)?
            .into_iter()
            .filter(|f| f.ends_with(".yml") || f.ends_with(".yaml")),
    ) {
        if safe_path(root, &file)?.is_file() {
            inventory.add("project", "gate-source", &file);
        }
    }
    inventory
        .items
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(inventory)
}

pub(crate) fn plan(root: &Path) -> Result<Value> {
    let items = scan(root)?.items;
    Ok(
        json!({"conflicts":items.iter().filter(|item|item.category=="conflict").collect::<Vec<_>>(),"suggestedGates":init::detect(root)?,"suggestedMemorySources":items.iter().filter(|item|item.category=="memory-source").map(|item|&item.path).collect::<Vec<_>>(),"ruleSources":items.iter().filter(|item|(item.category=="rule-source"||item.path==".cursorrules") && root.join(&item.path).is_file()).collect::<Vec<_>>()}),
    )
}

pub(crate) fn cleanup(root: &Path, state: Option<&Path>, dry_run: bool) -> Result<Value> {
    let conflicts: Vec<_> = scan(root)?
        .items
        .into_iter()
        .filter(|item| item.category == "conflict")
        .collect();
    let hooks: Vec<_> = conflicts
        .iter()
        .filter(|item| {
            HOOKS.iter().any(|dir| {
                item.path
                    .strip_prefix(dir)
                    .is_some_and(|suffix| suffix.starts_with('/'))
            })
        })
        .map(|item| item.path.clone())
        .collect();
    let mut plan = Plan::default();
    for item in conflicts {
        let file = item.path.as_str();
        let path = safe_path(root, file)?;
        if path.is_dir() {
            plan.tree(root, file)?;
        } else if matches!(file, "AGENTS.md" | "CLAUDE.md") {
            let text = fs::read_to_string(path)?;
            if let Some(body) = init::strip(&text, "TRELLIS", file)? {
                plan.edit(
                    file,
                    text.as_bytes(),
                    (!body.is_empty()).then(|| body.into_bytes()),
                );
            }
        } else if SETTINGS.contains(&file) {
            let bytes = fs::read(path)?;
            let mut data: Value = serde_json::from_slice(&bytes)
                .with_context(|| format!("Cannot safely edit {file}"))?;
            ensure!(
                clean_settings(&mut data, &references(file, &hooks)?),
                "Cannot safely isolate Trellis hooks in {file}"
            );
            let replacement = if data.as_object().is_some_and(|s| s.is_empty()) {
                None
            } else {
                Some(format!("{}\n", serde_json::to_string_pretty(&data)?).into_bytes())
            };
            plan.edit(file, &bytes, replacement);
        } else {
            plan.remove(file);
        }
    }
    snapshot::apply(root, Operation::Migrate, plan, state, dry_run)
}
