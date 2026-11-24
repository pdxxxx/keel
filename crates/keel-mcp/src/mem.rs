use crate::config;
use crate::paths::{ascii_slug, display, js_whitespace, relative, resolve, trim};
use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Value, value::RawValue};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};
use walkdir::WalkDir;

struct Document {
    source: &'static str,
    path: PathBuf,
    modified_ms: i64,
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

fn documents(root: &Path, source: Option<&str>) -> Result<Vec<Document>> {
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

    let mut files = Vec::new();
    for (source, dir) in dirs {
        for entry in WalkDir::new(dir)
            .min_depth(1)
            .max_depth(6)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let ext = entry.path().extension().and_then(|ext| ext.to_str());
            if !matches!(
                (source, ext),
                ("keel" | "custom", Some("md"))
                    | ("custom" | "claude" | "codex" | "opencode", Some("jsonl"))
            ) {
                continue;
            }
            let time = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(UNIX_EPOCH);
            let rounded = time.checked_add(Duration::from_micros(500)).unwrap_or(time);
            files.push(Document {
                source,
                path: entry.into_path(),
                modified_ms: DateTime::<Utc>::from(rounded).timestamp_millis(),
            });
        }
    }
    files.sort_by_key(|doc| std::cmp::Reverse(doc.modified_ms));
    Ok(files)
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

fn json_turn(line: &str, visit: &mut impl FnMut(&'static str, &str, Option<&str>)) {
    if !line.starts_with('{') {
        return;
    }
    let Ok(outer) = serde_json::from_str::<Frame>(line) else {
        return;
    };
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

fn turns(doc: &Document, mut visit: impl FnMut(&'static str, &str, Option<&str>)) {
    if doc.path.extension().is_some_and(|ext| ext == "md") {
        if let Ok(text) = fs::read_to_string(&doc.path) {
            let text = trim(&text);
            if !text.is_empty() {
                visit("artifact", text, None);
            }
        }
        return;
    }
    let Ok(file) = File::open(&doc.path) else {
        return;
    };
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if let Ok(text) = std::str::from_utf8(&line) {
                    json_turn(trim(text), &mut visit);
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
    title: Utf16Text,
    updated_at: String,
    turn_count: usize,
}

#[derive(Default)]
struct Utf16Text(Vec<u16>);

impl Serialize for Utf16Text {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Ok(text) = String::from_utf16(&self.0) {
            return serializer.serialize_str(&text);
        }
        let mut json = String::from("\"");
        let mut run = String::new();
        for decoded in char::decode_utf16(self.0.iter().copied()) {
            match decoded {
                Ok(character) => run.push(character),
                Err(error) => {
                    let escaped = serde_json::to_string(&run).map_err(serde::ser::Error::custom)?;
                    json.push_str(&escaped[1..escaped.len() - 1]);
                    json.push_str(&format!("\\u{:04x}", error.unpaired_surrogate()));
                    run.clear();
                }
            }
        }
        let escaped = serde_json::to_string(&run).map_err(serde::ser::Error::custom)?;
        json.push_str(&escaped[1..]);
        RawValue::from_string(json)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

fn title(text: &str, path: &Path) -> Utf16Text {
    let Some(line) = text
        .lines()
        .map(trim)
        .find(|line| !line.is_empty() && *line != "---")
    else {
        return Utf16Text(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .encode_utf16()
                .collect(),
        );
    };
    let line = line
        .trim_start_matches('#')
        .trim_start_matches(js_whitespace);
    Utf16Text(line.encode_utf16().take(80).collect())
}

pub(crate) fn sessions(root: &Path, source: Option<&str>, limit: usize) -> Result<Vec<Session>> {
    let mut sessions = Vec::new();
    for doc in documents(root, source)? {
        if sessions.len() >= limit {
            break;
        }
        let mut count = 0;
        let mut first_user = false;
        let mut name = Utf16Text::default();
        turns(&doc, |role, text, _| {
            if count == 0 || (role == "user" && !first_user) {
                name = title(text, &doc.path);
                first_user = role == "user";
            }
            count += 1;
        });
        if count != 0 {
            sessions.push(Session {
                id: doc.id(),
                source: doc.source,
                file: doc.file(root),
                title: name,
                updated_at: DateTime::from_timestamp_millis(doc.modified_ms)
                    .unwrap_or(DateTime::UNIX_EPOCH)
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                turn_count: count,
            });
        }
    }
    Ok(sessions)
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Score(u16);

impl Serialize for Score {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0 % 100 == 0 {
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
    if covered == 0 {
        return None;
    }
    let score = covered as f64 / terms.len() as f64 * 10.0 + occurrences as f64 * 0.5;
    Some((Score((score * 100.0).round() as u16), first))
}

fn snippet(text: &str, lower: &str, first: usize) -> Utf16Text {
    let center = lower[..first].encode_utf16().count();
    let start = center.saturating_sub(120);
    let mut units = text.encode_utf16().skip(start);
    let mut excerpt = Vec::new();
    if start > 0 {
        excerpt.extend("...".encode_utf16());
    }
    let mut in_text = false;
    let mut pending_space = false;
    for unit in units.by_ref().take(center + 120 - start) {
        if char::from_u32(u32::from(unit)).is_some_and(js_whitespace) {
            pending_space = in_text;
        } else {
            if pending_space {
                excerpt.push(u16::from(b' '));
            }
            pending_space = false;
            in_text = true;
            excerpt.push(unit);
        }
    }
    if units.next().is_some() {
        excerpt.extend("...".encode_utf16());
    }
    Utf16Text(excerpt)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Hit {
    session_id: String,
    source: &'static str,
    file: String,
    role: &'static str,
    score: Score,
    snippet: Utf16Text,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
}

struct RankedHit {
    hit: Hit,
    order: usize,
}

impl Ord for RankedHit {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .hit
            .score
            .cmp(&self.hit.score)
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
    for doc in documents(root, source)? {
        if limit == 0 {
            break;
        }
        let mut identity = None;
        turns(&doc, |role, text, timestamp| {
            let lower = text.to_lowercase();
            let Some((score, first)) = rank(&lower, &terms) else {
                return;
            };
            if best.len() == limit {
                if best.peek().is_some_and(|worst| worst.hit.score >= score) {
                    return;
                }
                best.pop();
            }
            let (id, file) = identity.get_or_insert_with(|| (doc.id(), doc.file(root)));
            let hit = Hit {
                session_id: id.clone(),
                source: doc.source,
                file: file.clone(),
                role,
                score,
                snippet: snippet(text, &lower, first),
                timestamp: timestamp.map(str::to_owned),
            };
            best.push(RankedHit { hit, order });
            order += 1;
        });
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
    fn snippet_uses_utf16_offsets_and_collapses_whitespace() {
        let text = format!(
            "{}\t🚀  Two\nWORDS   {}",
            "甲".repeat(130),
            "乙".repeat(130)
        );
        let lower = text.to_lowercase();
        let (_, first) = rank(&lower, &["words", "two"]).unwrap();
        let snippet = String::from_utf16(&snippet(&text, &lower, first).0).unwrap();
        assert!(snippet.starts_with("...甲"));
        assert!(snippet.contains("🚀 Two WORDS 乙"));
        assert!(snippet.ends_with("乙..."));
        assert_eq!(snippet.matches('甲').count(), 115);
    }

    #[test]
    fn sliced_surrogates_are_json_escapes_not_replacement_characters() {
        let units = Utf16Text(vec![0xdc00, 34, 92, 10, 0xd83d]);
        assert_eq!(
            serde_json::to_string(&units).unwrap(),
            r#""\udc00\"\\\n\ud83d""#
        );
    }

    #[test]
    fn turns_skip_bad_bytes_and_use_only_recognized_roles() {
        let temp = tempfile::tempdir().unwrap();
        let doc = Document {
            source: "codex",
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
        turns(&doc, |role, text, timestamp| {
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
