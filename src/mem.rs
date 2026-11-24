use crate::config;
use crate::paths::{ascii_slug, display, js_whitespace, relative, resolve, trim};
use anyhow::{Result, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{
    Deserialize, Serialize, Serializer,
    de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, value::RawValue};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet, VecDeque};
use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};
use walkdir::WalkDir;

struct Document {
    source: &'static str,
    path: PathBuf,
    modified_ms: i64,
    claude_dir: Option<String>,
}

impl Document {
    fn id(&self) -> String {
        let native = self.path.to_string_lossy();
        #[cfg(windows)]
        let native = native.replace('/', "\\");
        let digest = Sha256::digest(native.as_bytes());
        let stem = self.path.file_stem().unwrap_or_default().to_string_lossy();
        let base = ascii_slug(&stem, "_-");
        let base = if base.is_empty() {
            "doc"
        } else {
            &base[..base.len().min(24)]
        };
        format!(
            "{}:{base}-{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.source, digest[0], digest[1], digest[2], digest[3], digest[4]
        )
    }

    fn file(&self, root: &Path) -> String {
        if self.source == "keel" {
            relative(root, &self.path)
        } else {
            display(&self.path)
        }
    }
}

fn locations(root: &Path, source: Option<&str>) -> Result<Vec<(&'static str, PathBuf)>> {
    let config = config::load(root)?;
    let selected = |name: &str| source.is_none_or(|filter| filter.is_empty() || filter == name);
    let mut dirs = Vec::new();
    if selected("keel") {
        dirs.extend([
            ("keel", root.join(".keel/tasks")),
            ("keel", root.join(".keel/rules")),
        ]);
    }
    let home = env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(env::home_dir);
    if let Some(home) = home {
        for (name, directory) in [
            ("claude", ".claude/projects"),
            ("codex", ".codex/sessions"),
            ("opencode", ".local/share/opencode"),
            ("antigravity", ".gemini/antigravity/brain"),
            ("gemini", ".gemini/tmp"),
        ] {
            if selected(name) {
                dirs.push((name, resolve(&home, Path::new(directory))?));
            }
        }
    }
    if selected("custom") {
        let mut seen = HashSet::new();
        let mut add = |path| {
            if seen.insert(PathBuf::clone(&path)) {
                dirs.push(("custom", path));
            }
        };
        for source in config.sources {
            add(resolve(root, Path::new(&source))?);
        }
        if let Some(value) = env::var_os("KEEL_MEM_DIRS") {
            for dir in env::split_paths(&value) {
                let text = dir.to_string_lossy();
                let text = trim(&text);
                if !text.is_empty() {
                    add(resolve(Path::new("."), Path::new(text))?);
                }
            }
        }
    }

    Ok(dirs)
}

fn source_documents(source: &'static str, dir: PathBuf) -> impl Iterator<Item = Document> {
    WalkDir::new(&dir)
        .min_depth(1)
        .max_depth(match source {
            "gemini" => 3,
            "antigravity" => 4,
            _ => usize::MAX,
        })
        .sort_by_file_name()
        .into_iter()
        .filter_entry(move |entry| match (source, entry.depth()) {
            ("gemini", 2) => entry.file_name() == "chats",
            ("gemini", 3) | ("antigravity", 4) => entry.file_type().is_file(),
            ("antigravity", 2) => entry.file_name() == ".system_generated",
            ("antigravity", 3) => entry.file_name() == "logs",
            _ => true,
        })
        .filter_map(Result::ok)
        .filter_map(move |entry| {
            if !entry.file_type().is_file() {
                return None;
            }
            let ext = entry.path().extension().and_then(|ext| ext.to_str());
            let relative = entry.path().strip_prefix(&dir).unwrap();
            let mut parts = relative.iter();
            let first = parts.next();
            let special = match source {
                "antigravity" => {
                    parts.as_path() == Path::new(".system_generated/logs/transcript.jsonl")
                }
                "gemini" => {
                    parts.next().is_some_and(|part| part == "chats")
                        && parts.count() == 1
                        && ext == Some("json")
                }
                _ => false,
            };
            if !special
                && !matches!(
                    (source, ext),
                    ("keel" | "custom", Some("md"))
                        | ("custom" | "claude" | "codex" | "opencode", Some("jsonl"))
                )
            {
                return None;
            }
            let time = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(UNIX_EPOCH);
            let rounded = time.checked_add(Duration::from_micros(500)).unwrap_or(time);
            let claude_dir = first
                .filter(|_| source == "claude")
                .map(|part| part.to_string_lossy().to_lowercase());
            Some(Document {
                source,
                claude_dir,
                path: entry.into_path(),
                modified_ms: DateTime::<Utc>::from(rounded).timestamp_millis(),
            })
        })
}

fn documents(root: &Path, source: Option<&str>) -> Result<impl Iterator<Item = Document> + use<>> {
    Ok(locations(root, source)?
        .into_iter()
        .flat_map(|(name, dir)| source_documents(name, dir)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Source {
    source: &'static str,
    session_count: usize,
    locations: Vec<String>,
}

pub(crate) fn sources(root: &Path, source: Option<&str>) -> Result<Vec<Source>> {
    let mut stats: Vec<Source> = Vec::new();
    for (name, dir) in locations(root, source)? {
        if stats.last().is_none_or(|stats| stats.source != name) {
            stats.push(Source {
                source: name,
                session_count: 0,
                locations: Vec::new(),
            });
        }
        if dir.is_dir()
            && let Some(stats) = stats.last_mut()
        {
            stats.locations.push(display(&dir));
            stats.session_count += source_documents(name, dir)
                .filter(|doc| detail(root, doc, false, 0).is_some())
                .count();
        }
    }
    Ok(stats)
}

#[derive(Deserialize, Default)]
struct Frame<'line> {
    #[serde(borrow)]
    message: Option<&'line RawValue>,
    #[serde(borrow)]
    payload: Option<&'line RawValue>,
    #[serde(borrow, rename = "type")]
    kind: Option<&'line RawValue>,
    #[serde(borrow)]
    role: Option<&'line RawValue>,
    #[serde(borrow)]
    content: Option<&'line RawValue>,
    #[serde(borrow)]
    text: Option<&'line RawValue>,
    #[serde(borrow)]
    timestamp: Option<&'line RawValue>,
    #[serde(borrow)]
    created_at: Option<&'line RawValue>,
}

fn string(raw: Option<&RawValue>) -> Option<Cow<'_, str>> {
    let raw = raw?.get();
    let unquoted = raw.strip_prefix('"')?.strip_suffix('"')?;
    if unquoted.contains('\\') {
        serde_json::from_str(raw).ok().map(Cow::Owned)
    } else {
        Some(Cow::Borrowed(unquoted))
    }
}

fn content(raw: &RawValue) -> Option<String> {
    if let Some(text) = string(Some(raw)) {
        let text = trim(&text);
        return (!text.is_empty()).then(|| text.to_owned());
    }
    if !raw.get().starts_with('[') {
        return None;
    }
    let blocks: Vec<Value> = serde_json::from_str(raw.get()).ok()?;
    let text = blocks
        .iter()
        .filter_map(|block| {
            block
                .as_str()
                .or_else(|| block["text"].as_str())
                .or_else(|| block["content"].as_str())
        })
        .collect::<Vec<_>>()
        .join("\n");
    let text = trim(&text);
    (!text.is_empty()).then(|| text.to_owned())
}

fn json_turn(source: &str, line: &str, visit: &mut impl FnMut(&'static str, &str, Option<&str>)) {
    if !line.starts_with('{') {
        return;
    }
    let Ok(outer) = serde_json::from_str::<Frame>(line) else {
        return;
    };
    if source == "antigravity" {
        let role = match string(outer.kind).as_deref() {
            Some("USER_INPUT") => "user",
            Some("PLANNER_RESPONSE") => "assistant",
            _ => return,
        };
        if let Some(text) = outer.content.and_then(content) {
            let stamp = string(outer.created_at);
            visit(
                role,
                &text,
                stamp.as_deref().filter(|text| !text.is_empty()),
            );
        }
        return;
    }
    let nested = [outer.message, outer.payload]
        .into_iter()
        .flatten()
        .find(|raw| raw.get().starts_with(['{', '[']))
        .map(|raw| serde_json::from_str::<Frame>(raw.get()).unwrap_or_default());
    let message = nested.as_ref().unwrap_or(&outer);
    let role = match string(message.role)
        .or_else(|| string(outer.kind))
        .as_deref()
    {
        Some("user" | "human") => "user",
        Some("assistant" | "model") => "assistant",
        _ => return,
    };
    if let Some(text) = [message.content, message.text, outer.content, outer.text]
        .into_iter()
        .flatten()
        .find_map(content)
    {
        let timestamp = string(outer.timestamp).or_else(|| string(outer.created_at));
        visit(
            role,
            &text,
            timestamp.as_deref().filter(|stamp| !stamp.is_empty()),
        );
    }
}

fn normalized_path(value: &str) -> Option<String> {
    let decoded;
    let value = if value
        .get(..7)
        .is_some_and(|start| start.eq_ignore_ascii_case("file://"))
    {
        let url = url::Url::parse(value).ok()?;
        let path = percent_encoding::percent_decode_str(url.path())
            .decode_utf8()
            .ok()?;
        let path = if matches!(path.as_bytes(), [b'/',letter,b':',b'/',..] if letter.is_ascii_alphabetic())
        {
            &path[1..]
        } else {
            &path
        };
        decoded = url
            .host_str()
            .map_or_else(|| path.to_owned(), |host| format!("//{host}{path}"));
        &decoded
    } else {
        value
    };
    let value = value.replace('\\', "/").to_lowercase();
    let (prefix, rest) = if let Some(rest) = value.strip_prefix('/') {
        ("/", rest)
    } else if matches!(value.as_bytes(), [letter,b':',b'/',..] if letter.is_ascii_alphabetic()) {
        (&value[..2], &value[3..])
    } else {
        return None;
    };
    let mut parts = Vec::new();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    let relative = parts.join("/");
    Some(if prefix == "/" || relative.is_empty() {
        format!("{prefix}{relative}")
    } else {
        format!("{prefix}/{relative}")
    })
}

fn ownership(data: &Value, root: &str) -> Option<bool> {
    fn inspect(value: &Value, root: &str) -> Option<bool> {
        if let Some(text) = value.as_str() {
            let path = normalized_path(text)?;
            return Some(
                path == root
                    || path
                        .strip_prefix(root)
                        .is_some_and(|suffix| root.ends_with('/') || suffix.starts_with('/')),
            );
        }
        let mut known = None;
        if let Some(items) = value.as_array() {
            for item in items {
                if let Some(owner) = inspect(item, root) {
                    known = Some(known.unwrap_or(false) || owner);
                }
            }
        } else if value.is_object() {
            for key in ["path", "uri", "root", "fsPath"] {
                if let Some(owner) = value[key].as_str().and_then(|_| inspect(&value[key], root)) {
                    known = Some(known.unwrap_or(false) || owner);
                }
            }
        }
        known
    }
    let mut known = None;
    for record in [data, &data["payload"], &data["metadata"]] {
        for key in [
            "cwd",
            "workspace",
            "workspacePath",
            "workspaceRoot",
            "workspaceUri",
            "workspaceFolders",
            "workspaces",
            "projectPath",
        ] {
            if let Some(owner) = inspect(&record[key], root) {
                known = Some(known.unwrap_or(false) || owner);
            }
        }
    }
    known
}

fn gemini_text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return trim(text).to_owned();
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| {
            part.as_str()
                .or_else(|| part["text"].as_str())
                .or_else(|| part["content"].as_str())
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches(js_whitespace)
        .to_owned()
}

struct Messages<'a, F>(&'a mut F);

impl<'de, F: FnMut(&Value)> DeserializeSeed<'de> for Messages<'_, F> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, F: FnMut(&Value)> Visitor<'de> for Messages<'_, F> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a messages array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(msg) = seq.next_element::<Value>()? {
            (self.0)(&msg);
        }
        Ok(())
    }
}

struct Chat<'a, F> {
    root: &'a str,
    message: F,
}

impl<'de, F: FnMut(&Value)> Visitor<'de> for Chat<'_, F> {
    type Value = Option<bool>;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a Gemini chat object")
    }
    fn visit_map<A: MapAccess<'de>>(mut self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut record = Value::Object(Default::default());
        let mut found = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "messages" if found => return Err(de::Error::duplicate_field("messages")),
                "messages" => {
                    map.next_value_seed(Messages(&mut self.message))?;
                    found = true;
                }
                "cwd" | "workspace" | "workspacePath" | "workspaceRoot" | "workspaceUri"
                | "workspaceFolders" | "workspaces" | "projectPath" | "payload" | "metadata" => {
                    record[&key] = map.next_value()?
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        if !found {
            return Err(de::Error::missing_field("messages"));
        }
        Ok(ownership(&record, self.root))
    }
}

fn gemini_pass(
    reader: &mut impl BufRead,
    root: &str,
    message: impl FnMut(&Value),
) -> serde_json::Result<Option<bool>> {
    let mut parser = serde_json::Deserializer::from_reader(reader);
    let owner = de::Deserializer::deserialize_map(&mut parser, Chat { root, message })?;
    parser.end()?;
    Ok(owner)
}

fn text_chunks(reader: &mut impl Read, mut accept: impl FnMut(&str)) -> io::Result<()> {
    let mut buf = [0; 16 * 1024];
    let mut pending = 0;
    loop {
        let read = reader.read(&mut buf[pending..])?;
        if read == 0 {
            return if pending == 0 {
                Ok(())
            } else {
                Err(io::ErrorKind::InvalidData.into())
            };
        }
        let end = pending + read;
        let valid = match std::str::from_utf8(&buf[..end]) {
            Ok(text) => {
                accept(text);
                end
            }
            Err(err) if err.error_len().is_none() => {
                let valid = err.valid_up_to();
                accept(std::str::from_utf8(&buf[..valid]).map_err(io::Error::other)?);
                valid
            }
            Err(_) => return Err(io::ErrorKind::InvalidData.into()),
        };
        buf.copy_within(valid..end, 0);
        pending = end - valid;
    }
}

fn artifact_preview(file: &mut File, full: bool) -> io::Result<Option<String>> {
    let mut text = String::new();
    let mut prefix = String::new();
    let mut title_found = false;
    let mut nonempty = false;
    text_chunks(file, |chunk| {
        if full {
            text.push_str(chunk);
            return;
        }
        for ch in chunk.chars() {
            nonempty |= !js_whitespace(ch);
            if title_found {
                continue;
            }
            if ch == '\n' {
                let line = trim(&prefix);
                if !line.is_empty() && line != "---" {
                    title_found = true;
                } else {
                    prefix.clear();
                }
            } else if prefix.len() < 512 && (!prefix.is_empty() || !js_whitespace(ch)) {
                prefix.push(ch);
            }
        }
    })?;
    if full {
        Ok((!trim(&text).is_empty()).then_some(text))
    } else {
        Ok(nonempty.then_some(prefix))
    }
}

fn visit_document(
    doc: &Document,
    root: &str,
    full_artifact: bool,
    mut visit: impl FnMut(&'static str, &str, Option<&str>, bool),
) {
    let intrinsic = matches!(doc.source, "keel" | "custom");
    let Ok(mut file) = File::open(&doc.path) else {
        return;
    };
    if doc.path.extension().is_some_and(|ext| ext == "md") {
        if let Ok(Some(text)) = artifact_preview(&mut file, full_artifact) {
            visit("artifact", trim(&text), None, intrinsic);
        }
        return;
    }
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    if doc.source == "gemini" {
        let mut first_owner = None;
        let Ok(owner) = gemini_pass(&mut reader, root, |msg| {
            if first_owner.is_none() {
                first_owner = ownership(msg, root);
            }
        }) else {
            return;
        };
        if reader.rewind().is_err() {
            return;
        }
        let project = owner.or(first_owner).unwrap_or(false);
        let _ = gemini_pass(&mut reader, root, |msg| {
            let role = match msg["type"].as_str().or_else(|| msg["role"].as_str()) {
                Some("user") => "user",
                Some("gemini" | "model" | "assistant") => "assistant",
                _ => return,
            };
            let text = [
                &msg["content"]["parts"],
                &msg["content"],
                &msg["parts"],
                &msg["text"],
            ]
            .into_iter()
            .find(|value| !value.is_null())
            .map(gemini_text)
            .unwrap_or_default();
            if !text.is_empty() {
                visit(
                    role,
                    &text,
                    msg["timestamp"]
                        .as_str()
                        .or_else(|| msg["created_at"].as_str())
                        .filter(|s| !s.is_empty()),
                    project,
                );
            }
        });
        return;
    }
    let mut line = Vec::new();
    let mut project = None;
    if matches!(doc.source, "claude" | "codex" | "antigravity") {
        let inspect_lines = if doc.source == "antigravity" {
            usize::MAX
        } else {
            32
        };
        for _ in 0..inspect_lines {
            line.clear();
            if !matches!(reader.read_until(b'\n', &mut line), Ok(size) if size > 0) {
                break;
            }
            if let Ok(text) = std::str::from_utf8(&line)
                && let Ok(data) = serde_json::from_str::<Value>(trim(text))
            {
                project = ownership(&data, root);
            }
            if project.is_some() {
                break;
            }
        }
        if reader.rewind().is_err() {
            return;
        }
    }
    let project = project.unwrap_or_else(|| {
        intrinsic
            || doc.claude_dir.as_ref().is_some_and(|name| {
                let encoded: String = root
                    .encode_utf16()
                    .map(|unit| {
                        char::from_u32(u32::from(unit))
                            .filter(char::is_ascii_alphanumeric)
                            .unwrap_or('-')
                    })
                    .collect();
                *name == encoded
                    || (root.as_bytes().get(1) == Some(&b':')
                        && *name == format!("{}{}", &encoded[..1], &encoded[2..]))
            })
    });
    let mut emit = |role, text: &str, stamp: Option<&str>| visit(role, text, stamp, project);
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if let Ok(text) = std::str::from_utf8(&line) {
                    json_turn(doc.source, trim(text), &mut emit);
                }
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Session {
    id: String,
    source: &'static str,
    file: String,
    title: String,
    updated_at: String,
    turn_count: usize,
}

fn title(text: &str, path: &Path) -> String {
    let Some(line) = text
        .lines()
        .map(trim)
        .find(|line| !line.is_empty() && *line != "---")
    else {
        return path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
    };
    let line = line
        .trim_start_matches('#')
        .trim_start_matches(js_whitespace);
    line.chars().take(80).collect()
}

#[derive(Serialize)]
struct Turn {
    role: &'static str,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct Detail {
    summary: Session,
    turns: Vec<Turn>,
}

fn detail(root: &Path, doc: &Document, project_only: bool, turn_limit: usize) -> Option<Detail> {
    let mut turns = Vec::new();
    let mut count = 0;
    let mut first_user = false;
    let mut name = String::new();
    visit_document(
        doc,
        &normalized_path(&root.to_string_lossy()).unwrap_or_default(),
        turn_limit > 0,
        |role, text, stamp, project| {
            if project_only && !project {
                return;
            }
            if turns.len() < turn_limit {
                turns.push(Turn {
                    role,
                    text: text.to_owned(),
                    timestamp: stamp.map(str::to_owned),
                });
            }
            if count == 0 || (role == "user" && !first_user) {
                name = title(text, &doc.path);
                first_user = role == "user";
            }
            count += 1;
        },
    );
    (count != 0).then(|| Detail {
        turns,
        summary: Session {
            id: doc.id(),
            source: doc.source,
            file: doc.file(root),
            title: name,
            updated_at: DateTime::from_timestamp_millis(doc.modified_ms)
                .unwrap_or(DateTime::UNIX_EPOCH)
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            turn_count: count,
        },
    })
}

pub(crate) fn sessions(
    root: &Path,
    source: Option<&str>,
    limit: usize,
    project: bool,
) -> Result<Vec<Session>> {
    let mut best: Vec<(i64, Session)> = Vec::new();
    if limit == 0 {
        return Ok(Vec::new());
    }
    for doc in documents(root, source)? {
        if best.len() == limit
            && best
                .last()
                .is_some_and(|(modified, _)| doc.modified_ms < *modified)
        {
            continue;
        }
        if let Some(Detail { summary, .. }) = detail(root, &doc, project, 0) {
            let index = best.partition_point(|(modified, current)| {
                *modified > doc.modified_ms
                    || (*modified == doc.modified_ms && current.file <= summary.file)
            });
            if index < limit {
                best.insert(index, (doc.modified_ms, summary));
                if best.len() > limit {
                    best.pop();
                }
            }
        }
    }
    Ok(best.into_iter().map(|(_, summary)| summary).collect())
}

pub(crate) fn show(root: &Path, id: &str, limit: usize) -> Result<Detail> {
    let mut found = None;
    let mut ambiguous = false;
    for doc in documents(root, None)? {
        let key = doc.id();
        if key.ends_with(id)
            && let Some(result) = detail(root, &doc, false, limit)
        {
            if key == id {
                return Ok(result);
            }
            ambiguous |= found.is_some();
            found = Some(result);
        }
    }
    if ambiguous {
        bail!("Ambiguous memory session ID: {id}")
    }
    found.ok_or_else(|| anyhow::anyhow!("Memory session not found: {id}"))
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Score(u16);

impl Serialize for Score {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.is_multiple_of(100) {
            serializer.serialize_u16(self.0 / 100)
        } else {
            serializer.serialize_f64(f64::from(self.0) / 100.0)
        }
    }
}

fn rank(lower: &str, terms: &[&str]) -> Option<(Score, usize)> {
    let mut first = usize::MAX;
    let mut covered = 0;
    let mut occurrences = 0;
    for term in terms {
        let mut matches = lower.match_indices(term);
        if let Some((offset, _)) = matches.next() {
            first = first.min(offset);
            covered += 1;
            if occurrences < 10 {
                occurrences += 1;
                occurrences += matches.take(10 - occurrences).count();
            }
        }
    }
    (covered != 0).then(|| (score(covered, terms.len(), occurrences), first))
}

fn score(covered: usize, terms: usize, occurrences: usize) -> Score {
    Score(
        ((covered as f64 / terms as f64 * 10.0 + occurrences.min(10) as f64 * 0.5) * 100.0).round()
            as u16,
    )
}

fn artifact_hit(path: &Path, terms: &[&str]) -> Option<(Score, String)> {
    let mut file = File::open(path).ok()?;
    let mut counts = vec![0usize; terms.len()];
    let mut offsets = vec![0usize; terms.len()];
    let overlap = terms.iter().map(|s| s.len()).max()?;
    let mut carry = String::new();
    let mut history: VecDeque<char> = VecDeque::new();
    let mut excerpt: Vec<char> = Vec::new();
    let mut prefix = false;
    let mut after = 0;
    let mut has_match = false;
    let mut started = false;
    let mut nonwhite_end = 0usize;
    let mut position = 0usize;
    let mut consumed = 0usize;
    text_chunks(&mut file, |chunk| {
        let text = if started {
            chunk
        } else {
            chunk.trim_start_matches(js_whitespace)
        };
        started |= !text.is_empty();
        let lower = text.to_lowercase();
        let mut window = std::mem::take(&mut carry);
        let base = consumed - window.len();
        window.push_str(&lower);
        let mut first = None;
        for (index, term) in terms.iter().enumerate() {
            if counts[index] >= 10 {
                continue;
            }
            let start = offsets[index].saturating_sub(base);
            for (offset, _) in window[start..].match_indices(term) {
                let offset = start + offset;
                let end = base + offset + term.len();
                if end <= consumed {
                    continue;
                }
                first = Some(first.map_or(offset, |earliest: usize| earliest.min(offset)));
                counts[index] += 1;
                offsets[index] = end;
                if counts[index] == 10 {
                    break;
                }
            }
        }
        let units: Vec<char> = text.chars().collect();
        if !has_match && let Some(offset) = first {
            let center = history
                .len()
                .saturating_add(window[..offset].chars().count())
                .saturating_sub(window[..consumed - base].chars().count());
            let mut buffer: Vec<_> = history.iter().copied().collect();
            buffer.extend(&units);
            let start = center.saturating_sub(120).min(buffer.len());
            prefix = position - history.len() + start > 0;
            after = position - history.len() + center + 120;
            excerpt.extend_from_slice(&buffer[start..(center + 120).min(buffer.len())]);
            has_match = true;
        } else if has_match && position < after {
            excerpt.extend(units.iter().take(after - position));
        }
        if !has_match {
            history.extend(&units);
            let excess = history.len().saturating_sub(overlap.saturating_add(120));
            history.drain(..excess);
        }
        let last = text.trim_end_matches(js_whitespace);
        if !last.is_empty() {
            nonwhite_end = position + last.chars().count();
        }
        position += units.len();
        consumed += lower.len();
        let mut start = window.len().saturating_sub(overlap);
        while !window.is_char_boundary(start) {
            start += 1;
        }
        carry = window[start..].to_owned();
    })
    .ok()?;
    if !has_match {
        return None;
    }
    let mut text = excerpt
        .iter()
        .collect::<String>()
        .split(js_whitespace)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if prefix {
        text.insert_str(0, "...");
    }
    if nonwhite_end > after {
        text.push_str("...");
    }
    Some((
        score(
            counts.iter().filter(|n| **n != 0).count(),
            terms.len(),
            counts.iter().sum(),
        ),
        text,
    ))
}

fn snippet(text: &str, lower: &str, first: usize) -> String {
    let center = lower[..first].chars().count();
    let start = center.saturating_sub(120);
    let mut units = text.chars().skip(start);
    let mut excerpt = String::new();
    if start > 0 {
        excerpt.push_str("...");
    }
    let mut in_text = false;
    let mut pending_space = false;
    for unit in units.by_ref().take(center + 120 - start) {
        if js_whitespace(unit) {
            pending_space = in_text;
        } else {
            if pending_space {
                excerpt.push(' ');
            }
            pending_space = false;
            in_text = true;
            excerpt.push(unit);
        }
    }
    if units.next().is_some() {
        excerpt.push_str("...");
    }
    excerpt
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Hit {
    session_id: String,
    source: &'static str,
    file: String,
    role: &'static str,
    score: Score,
    snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
}

struct RankedHit {
    hit: Hit,
    modified_ms: i64,
    order: usize,
}

impl Ord for RankedHit {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .hit
            .score
            .cmp(&self.hit.score)
            .then_with(|| other.modified_ms.cmp(&self.modified_ms))
            .then_with(|| self.hit.file.cmp(&other.hit.file))
            .then_with(|| self.order.cmp(&other.order))
    }
}
impl PartialOrd for RankedHit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for RankedHit {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for RankedHit {}

pub(crate) fn search(
    root: &Path,
    query: &str,
    source: Option<&str>,
    limit: usize,
    project_only: bool,
) -> Result<Vec<Hit>> {
    let query = query.to_lowercase();
    let terms: Vec<_> = query
        .split(js_whitespace)
        .filter(|term| !term.is_empty())
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let mut best: BinaryHeap<RankedHit> = BinaryHeap::new();
    let mut order = 0;
    let normalized = normalized_path(&root.to_string_lossy()).unwrap_or_default();
    for doc in documents(root, source)? {
        if limit == 0 {
            break;
        }
        let mut identity = None;
        let mut add =
            |role, mut score: Score, snippet: String, timestamp: Option<&str>, project: bool| {
                if project_only && !project {
                    return;
                }
                if !project_only && project && !matches!(doc.source, "keel" | "custom") {
                    score.0 += 500;
                }
                let (id, file) = identity.get_or_insert_with(|| (doc.id(), doc.file(root)));
                let hit = Hit {
                    session_id: id.clone(),
                    source: doc.source,
                    file: file.clone(),
                    role,
                    score,
                    snippet,
                    timestamp: timestamp.map(str::to_owned),
                };
                let candidate = RankedHit {
                    hit,
                    order,
                    modified_ms: doc.modified_ms,
                };
                order += 1;
                if best.len() == limit {
                    if best.peek().is_some_and(|worst| candidate >= *worst) {
                        return;
                    }
                    best.pop();
                }
                best.push(candidate);
            };
        if doc.path.extension().is_some_and(|ext| ext == "md") {
            if let Some((score, snippet)) = artifact_hit(&doc.path, &terms) {
                add("artifact", score, snippet, None, true);
            }
        } else {
            visit_document(&doc, &normalized, false, |role, text, stamp, project| {
                if project_only && !project {
                    return;
                }
                let lower = text.to_lowercase();
                if let Some((score, first)) = rank(&lower, &terms) {
                    add(role, score, snippet(text, &lower, first), stamp, project);
                }
            });
        }
    }
    Ok(best
        .into_sorted_vec()
        .into_iter()
        .map(|ranked| ranked.hit)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn metadata_matches_components_across_path_styles_and_file_uris() {
        for value in [
            "C:\\Work\\Repo\\src",
            "file:///c%3A/WORK/Repo",
            "c:/work/other/../repo/",
        ] {
            assert_eq!(
                ownership(
                    &serde_json::json!({"metadata":{"workspace":{"uri":value}}}),
                    "c:/work/repo"
                ),
                Some(true)
            );
        }
        assert_eq!(
            ownership(
                &serde_json::json!({"payload":{"cwd":"C:/Work/Repo-other"}}),
                "c:/work/repo"
            ),
            Some(false)
        );
        assert_eq!(
            ownership(
                &serde_json::json!({"content":"/work/repo","cwd":"relative"}),
                "/work/repo"
            ),
            None
        );
        assert_eq!(
            ownership(
                &serde_json::json!({"workspaceFolders":[{"uri":"file:///somewhere"},{"uri":"file:///Work/Repo"}]}),
                "/work/repo"
            ),
            Some(true)
        );
    }

    #[test]
    fn score_rewards_coverage_and_caps_non_overlapping_frequency() {
        assert_eq!(
            rank("alpha beta beta", &["alpha", "beta"]).unwrap().0.0,
            1150
        );
        assert_eq!(rank("aaab", &["aa", "x", "y"]).unwrap().0.0, 383);
        assert_eq!(rank(&"aa".repeat(40), &["aa"]).unwrap().0.0, 1500);
        assert_eq!(rank("term", &["term", "term"]).unwrap().0.0, 1100);
        assert!(rank("missing", &["x"]).is_none());
    }

    #[test]
    fn snippet_preserves_unicode_and_collapses_whitespace() {
        let text = format!(
            "{}\t🚀  Two\nWORDS   {}",
            "甲".repeat(130),
            "乙".repeat(130)
        );
        let lower = text.to_lowercase();
        let (_, first) = rank(&lower, &["words", "two"]).unwrap();
        let snippet = snippet(&text, &lower, first);
        assert!(snippet.starts_with("...甲"));
        assert!(snippet.contains("🚀 Two WORDS 乙"));
        assert!(snippet.ends_with("乙..."));
        assert_eq!(snippet.matches('甲').count(), 116);
    }

    #[test]
    fn title_truncation_never_splits_a_unicode_scalar() {
        let text = format!("{}🚀tail", "甲".repeat(79));
        let title = title(&text, Path::new("note.md"));
        assert!(title.ends_with('🚀'));
        assert_eq!(title.chars().count(), 80);
    }

    #[test]
    fn markdown_search_crosses_utf8_and_buffer_boundaries_without_changing_scores() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("notes.md");
        for gap in [0, 1, 2, 3, 4, 50, 16_379, 16_383, 16_384, 32_767] {
            let text = format!(
                "  {}🚀needle🦀  Beta needle\n{}",
                "a".repeat(gap),
                "尾".repeat(199)
            );
            fs::write(&file, &text).unwrap();
            let text = trim(&text);
            let lower = text.to_lowercase();
            let terms = ["🚀needle🦀", "beta", "missing", "aa"];
            let (expected, first) = rank(&lower, &terms).unwrap();
            let (actual, excerpt) = artifact_hit(&file, &terms).unwrap();
            assert_eq!(actual.0, expected.0, "gap {gap}");
            assert_eq!(excerpt, snippet(text, &lower, first));
            let terms = ["🚀needle🦀", "beta", "needle"];
            let (expected, first) = rank(&lower, &terms).unwrap();
            let (actual, excerpt) = artifact_hit(&file, &terms).unwrap();
            assert_eq!(actual.0, expected.0, "gap {gap}");
            assert_eq!(excerpt, snippet(text, &lower, first));
        }
        let mut corrupt = b"needle".repeat(5000);
        corrupt.push(255);
        fs::write(&file, corrupt).unwrap();
        assert!(artifact_hit(&file, &["needle"]).is_none());
    }

    #[test]
    fn turns_skip_bad_bytes_and_use_only_recognized_roles() {
        let temp = tempfile::tempdir().unwrap();
        let doc = Document {
            source: "codex",
            claude_dir: None,
            path: temp.path().join("session.jsonl"),
            modified_ms: 0,
        };
        let mut bytes = b"[]\nnot json\n{\"role\":\"user\",\"text\":\"\xff\"}\n".to_vec();
        bytes.extend_from_slice(
            br#"{"type":"assistant","message":{"role":"tool","text":"ignored"}}
{"created_at":"T","payload":{"role":"human","content":["one",{"text":"two"},{"content":"three"}]}}
{"type":"model","message":{},"content":" fallback "}"#,
        );
        fs::write(&doc.path, bytes).unwrap();
        let mut parsed = Vec::new();
        visit_document(&doc, "/repo", false, |role, text, timestamp, _| {
            parsed.push((role, text.to_owned(), timestamp.map(str::to_owned)))
        });
        assert_eq!(
            parsed,
            [
                ("user", "one\ntwo\nthree".into(), Some("T".into())),
                ("assistant", "fallback".into(), None)
            ]
        );
    }
}
