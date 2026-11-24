use crate::paths::{js_whitespace, posix, read_text, relative, trim};
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use yaml_serde::Value;

#[derive(Serialize)]
pub(crate) struct Rule {
    id: String,
    file: String,
    summary: String,
    severity: &'static str,
    paths: Vec<String>,
    global: bool,
}

#[derive(Serialize)]
struct Issue {
    file: String,
    message: String,
}

#[derive(Serialize)]
pub(crate) struct Validation {
    pub valid: bool,
    checked: usize,
    issues: Vec<Issue>,
}

fn files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.try_exists()? {
        for entry in WalkDir::new(dir) {
            let entry = entry?;
            if entry.file_type().is_file() && entry.file_name().to_string_lossy().ends_with(".md") {
                files.push(entry.into_path());
            }
        }
    }
    files.sort_by(|left, right| {
        left.to_string_lossy()
            .encode_utf16()
            .cmp(right.to_string_lossy().encode_utf16())
    });
    Ok(files)
}

fn frontmatter(text: &str) -> Option<(Result<Value, yaml_serde::Error>, &str)> {
    let rest = text
        .strip_prefix("---\r\n")
        .or_else(|| text.strip_prefix("---\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if matches!(line, "---\n" | "---\r\n" | "---") {
            let header = &rest[..offset];
            let header = header
                .strip_suffix("\r\n")
                .or_else(|| header.strip_suffix('\n'))
                .unwrap_or(header);
            return Some((yaml_serde::from_str(header), &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

fn field<'data>(data: &'data Value, key: &str) -> Option<&'data str> {
    data[key]
        .as_str()
        .map(trim)
        .filter(|value| !value.is_empty())
}

fn severity(value: &str) -> Option<&'static str> {
    match value {
        "invariant" => Some("invariant"),
        "guideline" => Some("guideline"),
        "adr" => Some("adr"),
        _ => None,
    }
}

pub(crate) fn list(root: &Path) -> Result<Vec<Rule>> {
    let dir = root.join(".keel/rules");
    let mut rules = Vec::new();
    for path in files(&dir)? {
        let name = relative(&dir, &path);
        let text = read_text(&path)?;
        let (data, body) = frontmatter(&text)
            .map(|(parsed, body)| (parsed.unwrap_or(Value::Null), body))
            .unwrap_or((Value::Null, &text));
        if name == "index.md" && data["id"].as_str().is_none() {
            continue;
        }
        let paths: Vec<_> = data["paths"]
            .as_sequence()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(trim)
            .filter(|path| !path.is_empty())
            .map(posix)
            .collect();
        rules.push(Rule {
            id: field(&data, "id").map_or_else(
                || name.strip_suffix(".md").unwrap_or(&name).replace('/', ":"),
                str::to_owned,
            ),
            file: relative(root, &path),
            summary: field(&data, "summary")
                .unwrap_or_else(|| {
                    body.lines()
                        .map(trim)
                        .find(|line| !line.is_empty())
                        .unwrap_or("")
                        .trim_start_matches('#')
                        .trim_start_matches(js_whitespace)
                })
                .to_owned(),
            severity: field(&data, "severity")
                .and_then(severity)
                .unwrap_or("guideline"),
            global: paths.is_empty(),
            paths,
        });
    }
    Ok(rules)
}

fn glob_units(text: &str) -> Cow<'_, str> {
    if text.chars().all(|character| character.len_utf16() == 1) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.encode_utf16()
            .map(|unit| {
                char::from_u32(u32::from(unit))
                    .unwrap_or_else(|| char::from_u32(0xf0000 + u32::from(unit - 0xd800)).unwrap())
            })
            .collect(),
    )
}

fn compile_glob(pattern: &str) -> Result<Regex> {
    let normalized = posix(pattern);
    let units = glob_units(normalized.trim_start_matches('/'));
    let mut chars = units.chars().peekable();
    let mut expression = String::from(r"\A");
    while let Some(character) = chars.next() {
        match character {
            '*' if chars.next_if_eq(&'*').is_some() => {
                if chars.next_if_eq(&'/').is_some() {
                    expression.push_str(r"(?:[^\r\n\x{2028}\x{2029}]*/)?");
                } else {
                    expression.push_str(r"[^\r\n\x{2028}\x{2029}]*");
                }
            }
            '*' => expression.push_str("[^/]*"),
            '?' => expression.push_str("[^/]"),
            literal => expression.push_str(&regex::escape(literal.encode_utf8(&mut [0; 4]))),
        }
    }
    expression.push_str(r"\z");
    Ok(Regex::new(&expression)?)
}

pub(crate) fn matching(root: &Path, targets: &[String], include_global: bool) -> Result<Vec<Rule>> {
    let normalized: Vec<_> = targets.iter().map(|target| posix(&posix(target))).collect();
    let targets: Vec<_> = normalized
        .iter()
        .map(|target| glob_units(target.trim_start_matches('/')))
        .collect();
    let mut matched = Vec::new();
    for rule in list(root)? {
        if rule.global {
            if include_global {
                matched.push(rule);
            }
        } else if !targets.is_empty() {
            for pattern in &rule.paths {
                let matcher = compile_glob(pattern)?;
                if targets.iter().any(|target| matcher.is_match(target)) {
                    matched.push(rule);
                    break;
                }
            }
        }
    }
    Ok(matched)
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Sequence(items) => items
            .iter()
            .map(|item| {
                if item.is_null() {
                    String::new()
                } else {
                    js_string(item)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Mapping(_) => "[object Object]".into(),
        Value::Tagged(tagged) => js_string(&tagged.value),
    }
}

pub(crate) fn check(root: &Path) -> Result<Validation> {
    let dir = root.join(".keel/rules");
    let mut issues = Vec::new();
    let mut checked = 0;
    let mut seen = HashMap::new();
    for path in files(&dir)? {
        let text = read_text(&path)?;
        let parsed = frontmatter(&text);
        if relative(&dir, &path) == "index.md" && parsed.is_none() {
            continue;
        }
        checked += 1;
        let file = relative(root, &path);
        let mut issue = |message| {
            issues.push(Issue {
                file: file.clone(),
                message,
            })
        };
        let data = match parsed {
            None => {
                issue("Missing YAML frontmatter (expected id, summary, severity).".into());
                continue;
            }
            Some((Err(error), _)) => {
                issue(format!("Invalid YAML frontmatter: {error}"));
                continue;
            }
            Some((Ok(data), _)) => data,
        };
        match field(&data, "id") {
            None => issue("Missing non-empty `id`.".into()),
            Some(id) => {
                if let Some(first) = seen.get(id) {
                    issue(format!(
                        "Duplicate rule id `{id}` (already defined in {first})."
                    ));
                } else {
                    seen.insert(id.to_owned(), file.clone());
                }
            }
        }
        if field(&data, "summary").is_none() {
            issue("Missing non-empty `summary`.".into());
        }
        if let Some(value) = data.get("severity") {
            if value.as_str().map(trim).and_then(severity).is_none() {
                issue(format!(
                    "Invalid severity `{}` (expected invariant | guideline | adr).",
                    js_string(value)
                ));
            }
        }
        if data.get("paths").is_some_and(|value| !value.is_sequence()) {
            issue("`paths` must be an array of glob strings.".into());
        }
    }
    Ok(Validation {
        valid: issues.is_empty(),
        checked,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn frontmatter_closes_only_on_a_standalone_delimiter() {
        let header = [
            "---",
            "id: boundary",
            "---foo: harmless",
            "summary: |-",
            "  before",
            "  ---foo",
            "  after",
            "severity: adr",
            "---",
        ];
        for (newline, after, expected_body) in [
            ("\n", "\n# Body", "# Body"),
            ("\r\n", "\r\n# Body", "# Body"),
            ("\n", "", ""),
        ] {
            let rule = header.join(newline) + after;
            let (parsed, body) = frontmatter(&rule).unwrap();
            let data = parsed.unwrap();
            assert_eq!(data["---foo"].as_str(), Some("harmless"));
            assert_eq!(data["summary"].as_str(), Some("before\n---foo\nafter"));
            assert_eq!(data["severity"].as_str(), Some("adr"));
            assert_eq!(body, expected_body);
        }
        for closing in ["---foo", "--- \n", "---\r", "---\r# Body", "  ---\n"] {
            assert!(frontmatter(&format!("---\nid: boundary\nsummary: kept\n{closing}")).is_none());
        }
        let (parsed, body) = frontmatter("---\r\n---\r\n# Body").unwrap();
        assert!(parsed.unwrap().is_null());
        assert_eq!(body, "# Body");
    }

    #[test]
    fn glob_implements_keel_wildcards_not_shell_expansions() {
        for (pattern, path, expected) in [
            ("/src/**/a?.rs", "src/ab.rs", true),
            (r".\src\**\a?.rs", "src/nested/ab.rs", true),
            ("src/*", "src/sub/file", false),
            ("a**b", "a/sub/b", true),
            ("{a,b}[x]", "{a,b}[x]", true),
            ("{a,b}[x]", "ax", false),
            ("*", "line\nbreak", true),
            ("**", "line\nbreak", false),
            ("?", "界", true),
            ("?", "🚀", false),
            ("??", "🚀", true),
            ("🚀", "🚀", true),
        ] {
            assert_eq!(
                compile_glob(pattern).unwrap().is_match(&glob_units(path)),
                expected,
                "{pattern} {path}"
            );
        }
    }

    #[test]
    fn listing_falls_back_but_check_reports_each_problem() {
        let repo = tempfile::tempdir().unwrap();
        let dir = repo.path().join(".keel/rules");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("index.md"), "# Index").unwrap();
        fs::write(
            dir.join("a.md"),
            "---\r\nid: same\r\nsummary: First\r\n---\r\n",
        )
        .unwrap();
        fs::write(
            dir.join("b.md"),
            "---\nid: ' same '\nseverity: null\npaths: invalid\n---\n# Heading",
        )
        .unwrap();
        fs::write(dir.join("sub/c.md"), "\n## Legacy\n").unwrap();
        fs::write(dir.join("sub/d.md"), "---\nid: [bad\n---\n# Fallback").unwrap();
        let rules = list(repo.path()).unwrap();
        assert_eq!(rules.len(), 4);
        assert_eq!(rules[1].summary, "Heading");
        assert_eq!(rules[2].id, "sub:c");
        assert_eq!(rules[3].summary, "Fallback");
        let report = check(repo.path()).unwrap();
        assert_eq!(report.checked, 4);
        assert_eq!(report.issues.len(), 6);
        assert!(report.issues[0].message.contains(".keel/rules/a.md"));
        assert!(
            report.issues[5]
                .message
                .starts_with("Invalid YAML frontmatter:")
        );
    }
}
