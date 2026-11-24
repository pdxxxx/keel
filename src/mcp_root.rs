use crate::paths::{find_repo, resolve, trim};
use anyhow::{Result, bail, ensure};
use serde_json::Value;
use std::path::{Path, PathBuf};
use url::Url;

#[cfg(windows)]
use dunce::canonicalize;
#[cfg(not(windows))]
use std::fs::canonicalize;

fn real_directory(directory: &Path, raw: &str) -> Result<PathBuf> {
    match canonicalize(directory) {
        Ok(real) if real.is_dir() => Ok(real),
        _ => bail!("Invalid root directory: {raw}"),
    }
}

fn file_uri(raw: &str) -> Option<PathBuf> {
    if !raw.get(..7)?.eq_ignore_ascii_case("file://") {
        return None;
    }
    let url = Url::parse(raw).ok()?;
    let path = url.path().as_bytes();
    for (index, byte) in path.iter().enumerate() {
        if *byte == b'%' {
            let escape = path.get(index + 1..index + 3)?;
            if !escape.iter().all(u8::is_ascii_hexdigit)
                || escape.eq_ignore_ascii_case(b"2f")
                || (cfg!(windows) && escape.eq_ignore_ascii_case(b"5c"))
            {
                return None;
            }
        }
    }
    let directory = url.to_file_path().ok()?;
    directory.to_str()?;
    Some(directory)
}

pub struct McpRoots {
    cwd: PathBuf,
    startup: Option<PathBuf>,
    locked: Option<PathBuf>,
    workspace: Option<PathBuf>,
}

impl McpRoots {
    pub fn new(cwd: &Path, root: Option<&Path>, lock_root: bool) -> Result<Self> {
        let cwd = real_directory(cwd, &cwd.to_string_lossy())?;
        let startup = root
            .map(|path| real_directory(&cwd.join(path), &path.to_string_lossy()))
            .transpose()?;
        let locked = if lock_root {
            let base = startup
                .as_deref()
                .or_else(|| find_repo(&cwd))
                .unwrap_or(&cwd);
            Some(real_directory(
                base,
                &root.unwrap_or(base).to_string_lossy(),
            )?)
        } else {
            None
        };
        Ok(Self {
            cwd,
            startup,
            locked,
            workspace: None,
        })
    }

    fn check_lock(&self, directory: &Path, raw: &str) -> Result<()> {
        if let Some(locked) = &self.locked {
            ensure!(
                directory.starts_with(locked),
                "Root path escapes locked root: {raw}"
            );
        }
        Ok(())
    }

    fn discover(&self, directory: &Path, raw: &str) -> Result<PathBuf> {
        let real = real_directory(directory, raw)?;
        self.check_lock(&real, raw)?;
        let root = find_repo(&real).unwrap_or(&real);
        self.check_lock(root, raw)?;
        Ok(root.to_owned())
    }

    pub(super) fn initialize(&mut self, params: &Value) {
        if self.startup.is_some() {
            return;
        }
        self.workspace = None;
        for uri in [&params["rootUri"], &params["workspaceFolders"][0]["uri"]] {
            if let Some(root) = uri
                .as_str()
                .and_then(|raw| self.discover(&file_uri(raw)?, raw).ok())
            {
                self.workspace = Some(root);
                return;
            }
        }
    }

    pub(super) fn for_tool(&self, name: &str, raw: Option<&str>) -> Result<PathBuf> {
        let base = || {
            self.startup
                .as_deref()
                .or(self.workspace.as_deref())
                .or_else(|| find_repo(&self.cwd))
        };
        if let Some(raw) = raw.filter(|raw| !trim(raw).is_empty()) {
            let input = Path::new(trim(raw));
            if input.is_absolute() {
                return self.discover(input, raw);
            }
            let target = resolve(base().unwrap_or(&self.cwd), input)
                .map_err(|_| anyhow::anyhow!("Invalid root directory: {raw}"))?;
            return self.discover(&target, raw);
        }
        let base = base();
        if base.is_none()
            && matches!(
                name,
                "keel_task_create" | "keel_verify" | "keel_rules_add" | "keel_task_update"
            )
        {
            bail!(
                "No .keel/ or .git/ repository found from working directory; pass \"root\" pointing to the target repository."
            );
        }
        let directory = base.unwrap_or(&self.cwd);
        if self.locked.is_some() {
            let raw = directory.to_string_lossy();
            let real = real_directory(directory, &raw)?;
            self.check_lock(&real, &raw)?;
            return Ok(real);
        }
        Ok(directory.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn root_uri_and_first_workspace_folder_supply_a_session_default() {
        let home = tempdir().unwrap();
        let repo = tempdir().unwrap();
        let nested = repo.path().join("space #文%25");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir(repo.path().join(".keel")).unwrap();
        let uri = Url::from_directory_path(&nested).unwrap().to_string();
        #[cfg(windows)]
        let uri = format!(
            "file:///{}",
            uri.strip_prefix("file:///")
                .unwrap()
                .replacen(':', "%3A", 1)
        );
        let mut roots = McpRoots::new(home.path(), None, false).unwrap();
        let home_uri = Url::from_directory_path(home.path()).unwrap().to_string();
        roots.initialize(&json!({"rootUri": home_uri, "workspaceFolders": [{"uri":uri}]}));
        assert_eq!(
            roots.for_tool("keel_verify", None).unwrap(),
            canonicalize(home.path()).unwrap()
        );
        roots.initialize(
            &json!({"rootUri":"not a file URL", "workspaceFolders":[{"uri":uri},{"uri":home_uri}]}),
        );
        assert_eq!(
            roots.for_tool("keel_verify", None).unwrap(),
            canonicalize(repo.path()).unwrap()
        );
        assert_eq!(
            roots.for_tool("keel_verify", home.path().to_str()).unwrap(),
            canonicalize(home.path()).unwrap()
        );
        assert_eq!(
            roots.for_tool("keel_verify", Some(".")).unwrap(),
            canonicalize(repo.path()).unwrap()
        );
    }

    #[test]
    fn invalid_uri_or_missing_marker_does_not_authorize_writes() {
        let temp = tempdir().unwrap();
        let mut roots = McpRoots::new(temp.path(), None, false).unwrap();
        fs::write(temp.path().join(".keel"), "not a repository directory").unwrap();
        let existing = Url::from_directory_path(temp.path()).unwrap();
        for uri in [
            "https://host/test".to_owned(),
            format!("{existing}%zz"),
            format!("{existing}%2F"),
            format!("{existing}absent"),
        ] {
            roots.initialize(&json!({"rootUri":uri}));
            for name in ["keel_verify", "keel_task_create"] {
                assert_eq!(
                    roots.for_tool(name, Some(" ")).unwrap_err().to_string(),
                    "No .keel/ or .git/ repository found from working directory; pass \"root\" pointing to the target repository."
                );
            }
        }
        assert!(roots.for_tool("keel_task_list", None).is_ok());
        assert_eq!(
            roots
                .for_tool("keel_verify", Some(" .keel "))
                .unwrap_err()
                .to_string(),
            "Invalid root directory:  .keel "
        );
        fs::write(temp.path().join(".git"), "gitdir: elsewhere").unwrap();
        assert!(roots.for_tool("keel_verify", None).is_ok());
    }

    #[test]
    fn startup_beats_initialize_but_does_not_override_a_per_call_root() {
        let temp = tempdir().unwrap();
        let cwd = temp.path();
        let first = cwd.join("repo");
        let other = cwd.join("repo-other");
        fs::create_dir_all(first.join(".keel")).unwrap();
        fs::create_dir_all(other.join("src/.git")).unwrap();
        fs::create_dir_all(other.join(".keel")).unwrap();
        let mut roots = McpRoots::new(cwd, Some(Path::new("repo")), false).unwrap();
        roots.initialize(&json!({"rootUri":Url::from_directory_path(&other).unwrap().as_str()}));
        assert_eq!(
            roots
                .for_tool("keel_rules_list", Some("../repo-other/src"))
                .unwrap(),
            canonicalize(&other).unwrap()
        );
        assert_eq!(
            roots.for_tool("keel_rules_list", None).unwrap(),
            canonicalize(&first).unwrap()
        );
    }

    #[test]
    fn locked_roots_check_both_the_requested_directory_and_its_discovered_parent() {
        let temp = tempdir().unwrap();
        let first = temp.path().join("repo");
        let other = temp.path().join("repo-other");
        fs::create_dir_all(first.join("nested")).unwrap();
        fs::create_dir_all(other.join(".git")).unwrap();
        fs::create_dir(first.join(".keel")).unwrap();
        let roots = McpRoots::new(temp.path(), Some(&first), true).unwrap();
        assert_eq!(
            roots.for_tool("keel_verify", Some("nested")).unwrap(),
            canonicalize(&first).unwrap()
        );
        assert_eq!(
            roots
                .for_tool("keel_verify", Some("../repo-other"))
                .unwrap_err()
                .to_string(),
            "Root path escapes locked root: ../repo-other"
        );
        let nested = McpRoots::new(temp.path(), Some(&first.join("nested")), true).unwrap();
        assert_eq!(
            nested
                .for_tool("keel_task_create", Some("."))
                .unwrap_err()
                .to_string(),
            "Root path escapes locked root: ."
        );
    }
}
