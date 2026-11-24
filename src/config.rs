use crate::paths::{posix, trim};
use anyhow::{Result, ensure};
use std::collections::HashSet;
use std::path::Path;
use yaml_serde::Value;

#[derive(Debug, PartialEq)]
pub(crate) struct Gate {
    pub name: String,
    pub command: String,
    pub cwd: Option<String>,
    pub paths: Vec<String>,
    pub timeout_ms: u64,
    pub max_output_chars: usize,
}

#[derive(Debug, Default)]
pub(crate) struct Config {
    pub gates: Vec<Gate>,
    pub sources: Vec<String>,
}

pub(crate) fn glob_paths(value: &Value) -> Vec<String> {
    value
        .as_sequence()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(trim)
        .filter(|path| !path.is_empty())
        .map(posix)
        .collect()
}

#[derive(Default)]
struct Schema {
    errors: Vec<String>,
}

impl Schema {
    fn object(&mut self, data: &Value, path: &str, fields: &[&str]) -> bool {
        let Some(map) = data.as_mapping() else {
            self.errors.push(format!("{path}: expected a mapping"));
            return false;
        };
        for key in map.keys() {
            let key = key.as_str().unwrap_or("<non-string>");
            if !fields.contains(&key) {
                self.errors.push(format!("{path}.{key}: unknown field"));
            }
        }
        true
    }

    fn array<'a>(&mut self, data: &'a Value, key: &str, path: &str) -> &'a [Value] {
        match data.get(key) {
            Some(Value::Sequence(items)) => items,
            None => &[],
            Some(_) => {
                self.errors.push(format!("{path}.{key}: expected an array"));
                &[]
            }
        }
    }

    fn strings(&mut self, data: &Value, key: &str, path: &str) -> Vec<String> {
        self.array(data, key, path)
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                if let Some(text) = value.as_str().map(trim).filter(|s| !s.is_empty()) {
                    return Some(text.to_owned());
                }
                self.errors.push(format!(
                    "{path}.{key}[{index}]: expected a non-empty string"
                ));
                None
            })
            .collect()
    }

    fn text(&mut self, data: &Value, key: &str, path: &str) -> String {
        if let Some(text) = data[key].as_str().map(trim).filter(|s| !s.is_empty()) {
            return text.to_owned();
        }
        self.errors
            .push(format!("{path}.{key}: expected a non-empty string"));
        String::new()
    }

    fn limit(&mut self, data: &Value, key: &str, path: &str, default: u64) -> u64 {
        if let Some(value) = data.get(key) {
            if let Some(count) = value.as_u64().filter(|n| usize::try_from(*n).is_ok()) {
                return count;
            }
            self.errors
                .push(format!("{path}.{key}: expected a non-negative integer"));
        }
        default
    }
}

pub(crate) fn inspect(root: &Path) -> Result<(Config, Vec<String>)> {
    let raw = std::fs::read_to_string(root.join(".keel/config.yaml"))?;
    let data: Value = yaml_serde::from_str(&raw)?;
    let mut schema = Schema::default();
    let mut config = Config::default();
    let mut names = HashSet::new();
    schema.object(&data, "config", &["version", "verify", "memory"]);
    if data
        .get("version")
        .is_some_and(|value| value.as_u64() != Some(1))
    {
        schema
            .errors
            .push("config.version: expected schema version 1".into());
    }
    if let Some(verify) = data.get("verify")
        && schema.object(verify, "verify", &["gates"])
    {
        for (index, item) in schema.array(verify, "gates", "verify").iter().enumerate() {
            let path = format!("verify.gates[{index}]");
            if !schema.object(
                item,
                &path,
                &[
                    "name",
                    "command",
                    "cwd",
                    "paths",
                    "timeoutMs",
                    "maxOutputChars",
                ],
            ) {
                continue;
            }
            let name = schema.text(item, "name", &path);
            if !name.is_empty() && !names.insert(name.clone()) {
                schema
                    .errors
                    .push(format!("{path}.name: duplicate gate `{name}`"));
            }
            config.gates.push(Gate {
                name,
                command: schema.text(item, "command", &path),
                cwd: item.get("cwd").map(|_| schema.text(item, "cwd", &path)),
                paths: schema
                    .strings(item, "paths", &path)
                    .iter()
                    .map(|s| posix(s))
                    .collect(),
                timeout_ms: schema.limit(item, "timeoutMs", &path, 120_000),
                max_output_chars: schema.limit(item, "maxOutputChars", &path, 16_000) as usize,
            });
        }
    }
    if let Some(memory) = data.get("memory")
        && schema.object(memory, "memory", &["sources"])
    {
        config.sources = schema.strings(memory, "sources", "memory");
    }
    Ok((config, schema.errors))
}

pub(crate) fn load(root: &Path) -> Result<Config> {
    if !root.join(".keel/config.yaml").try_exists()? {
        return Ok(Config::default());
    }
    let (config, errors) = inspect(root)?;
    ensure!(
        errors.is_empty(),
        "Invalid .keel/config.yaml: {}",
        errors.join("; ")
    );
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn gate_limits_distinguish_zero_from_missing_and_invalid_values() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir(repo.path().join(".keel")).unwrap();
        fs::write(repo.path().join(".keel/config.yaml"), "verify:\n  gates:\n  - {name: none, command: echo, timeoutMs: 0, maxOutputChars: 0}\n  - {name: cap, command: echo}\n  - {name: custom, command: echo, timeoutMs: 25, maxOutputChars: 3}\n").unwrap();
        let limits: Vec<_> = load(repo.path())
            .unwrap()
            .gates
            .iter()
            .map(|gate| (gate.timeout_ms, gate.max_output_chars))
            .collect();
        assert_eq!(limits, [(0, 0), (120000, 16000), (25, 3)]);
    }

    #[test]
    fn gate_paths_use_rule_globs_and_empty_paths_are_global() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir(repo.path().join(".keel")).unwrap();
        fs::write(repo.path().join(".keel/config.yaml"), "verify:\n  gates:\n    - {name: api, command: run, paths: [' ./src\\api\\** ']}\n    - {name: empty, command: run, paths: []}\n    - {name: global, command: run}\n").unwrap();
        let gates = load(repo.path()).unwrap().gates;
        assert_eq!(gates[0].paths, ["src/api/**"]);
        assert!(gates[1].paths.is_empty() && gates[2].paths.is_empty());
    }

    #[test]
    fn invalid_config_reports_all_fields_before_any_gate_executes() {
        let temp = tempfile::tempdir().unwrap();
        assert!(load(temp.path()).unwrap().gates.is_empty());
        fs::create_dir(temp.path().join(".keel")).unwrap();
        let file = temp.path().join(".keel/config.yaml");
        fs::write(&file, "verify:\n  gates: [null, {name: ignored}, {name: ' ok ', command: ' echo ok ', cwd: ' '} ]\nmemory: {sources: [null, 123, ' ', ' notes ']}\n").unwrap();
        assert_eq!(inspect(temp.path()).unwrap().1.len(), 6);
        assert!(load(temp.path()).is_err());
        fs::write(&file, "verify: [unclosed").unwrap();
        assert!(load(temp.path()).is_err());
    }
}
