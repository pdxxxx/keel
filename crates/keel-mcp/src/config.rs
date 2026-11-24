use crate::paths::{posix, read_text, trim};
use anyhow::Result;
use std::path::Path;
use yaml_serde::Value;

#[derive(Debug, PartialEq)]
pub(crate) struct Gate {
    pub name: String,
    pub command: String,
    pub cwd: Option<String>,
    pub paths: Vec<String>,
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

pub(crate) fn load(root: &Path) -> Result<Config> {
    let path = root.join(".keel/config.yaml");
    if !path.try_exists()? {
        return Ok(Config::default());
    }
    let raw = read_text(&path)?;
    let data: Value = yaml_serde::from_str(&raw)?;
    let gates = data["verify"]["gates"]
        .as_sequence()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(Gate {
                name: trim(item["name"].as_str()?).to_owned(),
                command: trim(item["command"].as_str()?).to_owned(),
                paths: glob_paths(&item["paths"]),
                cwd: item["cwd"]
                    .as_str()
                    .map(trim)
                    .filter(|cwd| !cwd.is_empty())
                    .map(str::to_owned),
            })
        })
        .collect();
    let sources = data["memory"]["sources"]
        .as_sequence()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(trim)
        .filter(|source| !source.is_empty())
        .map(str::to_owned)
        .collect();
    Ok(Config { gates, sources })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn gate_paths_use_rule_globs_and_empty_paths_are_global() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir(repo.path().join(".keel")).unwrap();
        fs::write(repo.path().join(".keel/config.yaml"), "verify:\n  gates:\n    - {name: api, command: run, paths: [' ./src\\api\\** ', 1, ' ']}\n    - {name: empty, command: run, paths: []}\n    - {name: global, command: run}\n").unwrap();
        let gates = load(repo.path()).unwrap().gates;
        assert_eq!(gates[0].paths, ["src/api/**"]);
        assert!(gates[1].paths.is_empty() && gates[2].paths.is_empty());
    }

    #[test]
    fn config_is_optional_and_ignores_ill_typed_entries() {
        let temp = tempfile::tempdir().unwrap();
        assert!(load(temp.path()).unwrap().gates.is_empty());
        fs::create_dir(temp.path().join(".keel")).unwrap();
        let file = temp.path().join(".keel/config.yaml");
        fs::write(&file, "verify:\n  gates: [null, {name: ignored}, {name: ' ok ', command: ' echo ok ', cwd: ' '} ]\nmemory: {sources: [null, 123, ' ', ' notes ']}\n").unwrap();
        let config = load(temp.path()).unwrap();
        assert_eq!(
            config.gates,
            [Gate {
                name: "ok".into(),
                command: "echo ok".into(),
                cwd: None,
                paths: vec![]
            }]
        );
        assert_eq!(config.sources, ["notes"]);
        fs::write(&file, "verify: [unclosed").unwrap();
        assert!(load(temp.path()).is_err());
    }
}
