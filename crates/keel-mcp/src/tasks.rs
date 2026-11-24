use crate::paths::{ascii_slug, read_text, relative, trim};
use anyhow::{Result, bail};
use chrono::{Local, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    id: String,
    title: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Record {
    #[serde(flatten)]
    meta: Meta,
    dir: String,
    spec_file: String,
}

#[derive(Serialize)]
pub(crate) struct Detail {
    #[serde(flatten)]
    record: Record,
    spec: String,
}

fn record(root: &Path, dir: &Path, meta: Meta) -> Record {
    Record {
        meta,
        dir: relative(root, dir),
        spec_file: relative(root, &dir.join("spec.md")),
    }
}

fn read_meta(dir: &Path) -> Option<Meta> {
    let data: Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).ok()?).ok()?;
    let status = data["status"].as_str()?;
    if !matches!(status, "open" | "done") {
        return None;
    }
    Some(Meta {
        id: data["id"].as_str()?.to_owned(),
        title: data["title"].as_str()?.to_owned(),
        status: status.to_owned(),
        created_at: data["createdAt"].as_str().unwrap_or_default().to_owned(),
        updated_at: data["updatedAt"].as_str().unwrap_or_default().to_owned(),
    })
}

pub(crate) fn create(root: &Path, title: &str, slug: Option<&str>) -> Result<Record> {
    let title = trim(title);
    if title.is_empty() {
        bail!("Task title cannot be empty.");
    }
    let now = Local::now();
    let slug = ascii_slug(
        slug.map(trim)
            .filter(|slug| !slug.is_empty())
            .unwrap_or(title),
        "",
    );
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() {
        "task"
    } else {
        &slug[..slug.len().min(48)]
    };
    let base = format!("{}-{slug}", now.format("%Y-%m-%d"));
    let tasks = root.join(".keel/tasks");
    fs::create_dir_all(&tasks)?;
    let mut id = base.clone();
    let mut suffix = 2;
    let dir = loop {
        let dir = tasks.join(&id);
        match fs::create_dir(&dir) {
            Ok(()) => break dir,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                id = format!("{base}-{suffix}");
                suffix += 1;
            }
            Err(error) => return Err(error.into()),
        }
    };
    let timestamp = now
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    let meta = Meta {
        id,
        title: title.to_owned(),
        status: "open".into(),
        created_at: timestamp.clone(),
        updated_at: timestamp,
    };
    let spec = format!(
        "# {title}\n\n## Goal\n\n- What problem are we solving?\n\n## Non-Goals\n\n- What is explicitly out of scope?\n\n## Acceptance Criteria\n\n- [ ] Concrete, verifiable outcome\n\n## Decisions & Notes\n\n- Record tradeoffs or links to `research/` notes here so they survive session compaction.\n"
    );
    fs::write(dir.join("spec.md"), spec)?;
    fs::write(
        dir.join("meta.json"),
        format!("{}\n", serde_json::to_string_pretty(&meta)?),
    )?;
    Ok(record(root, &dir, meta))
}

pub(crate) fn list(root: &Path, status: Option<&str>, archived: bool) -> Result<Vec<Record>> {
    let tasks = root.join(".keel/tasks");
    let mut records = Vec::new();
    let mut dirs = vec![tasks.clone()];
    if archived {
        dirs.push(tasks.join("archive"));
    }
    for dir in dirs {
        if !dir.try_exists()? {
            continue;
        }
        let mut entries = fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if !entry.file_type()?.is_dir() || (dir == tasks && entry.file_name() == "archive") {
                continue;
            }
            let path = entry.path();
            if let Some(meta) = read_meta(&path) {
                if status.is_none_or(|status| status == meta.status) {
                    records.push(record(root, &path, meta));
                }
            }
        }
    }
    records.sort_by(|left, right| right.meta.created_at.cmp(&left.meta.created_at));
    Ok(records)
}

pub(crate) fn show(root: &Path, id: &str) -> Result<Detail> {
    let parts: Vec<_> = id
        .split(['/', '\\'])
        .filter(|part| !matches!(*part, "" | "."))
        .collect();
    if id.starts_with(['/', '\\'])
        || id.contains('\0')
        || parts.is_empty()
        || parts.iter().any(|part| {
            *part == ".."
                || matches!(part.as_bytes(), [letter, b':', ..] if letter.is_ascii_alphabetic())
        })
    {
        bail!("Task not found: {id}");
    }
    let mut dir = root.join(".keel/tasks");
    dir.extend(parts);
    let Some(meta) = read_meta(&dir) else {
        bail!("Task not found: {id}");
    };
    let spec = read_text(&dir.join("spec.md"))?;
    Ok(Detail {
        record: record(root, &dir, meta),
        spec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_matches_file_contract_and_reserves_unique_ids() {
        let repo = tempfile::tempdir().unwrap();
        let record = create(repo.path(), "  发布  ", None).unwrap();
        assert!(record.meta.id.ends_with("-task"));
        assert_eq!(record.meta.title, "发布");
        assert!(
            create(repo.path(), "发布", Some("  "))
                .unwrap()
                .meta
                .id
                .ends_with("-task-2")
        );
        let long = "X".repeat(60);
        let record = create(repo.path(), "long", Some(&long)).unwrap();
        assert_eq!(record.meta.id.len(), 11 + 48);
        assert_eq!(record.meta.created_at.len(), 24);
        let bytes = fs::read_to_string(repo.path().join(&record.dir).join("meta.json")).unwrap();
        assert!(bytes.starts_with("{\n  \"id\":"));
        assert!(bytes.ends_with("}\n"));
        assert_eq!(
            show(repo.path(), &record.meta.id).unwrap().spec,
            fs::read_to_string(repo.path().join(&record.spec_file)).unwrap()
        );
        assert!(show(repo.path(), "../outside").is_err());
    }

    #[test]
    fn archived_and_malformed_metadata_preserve_list_semantics() {
        let repo = tempfile::tempdir().unwrap();
        let tasks = repo.path().join(".keel/tasks");
        for (name, json) in [
            (
                "ok",
                r#"{"id":"ok","title":"Active","status":"open","createdAt":2}"#,
            ),
            ("bad", "{"),
            (
                "bad-status",
                r#"{"id":"x","title":"X","status":"archived"}"#,
            ),
            (
                "archive/old",
                r#"{"id":"old","title":"Old","status":"done","createdAt":"2025-01"}"#,
            ),
        ] {
            fs::create_dir_all(tasks.join(name)).unwrap();
            fs::write(tasks.join(name).join("meta.json"), json).unwrap();
        }
        let records = list(repo.path(), None, true).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].dir, ".keel/tasks/archive/old");
        assert_eq!(records[1].meta.created_at, "");
        assert!(list(repo.path(), Some("done"), false).unwrap().is_empty());
    }
}
