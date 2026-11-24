use crate::paths::{
    atomic_file, canonicalize, check_write, display, linked_entry, relative, resolve,
};
use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Operation {
    Ablate,
    Migrate,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::Ablate => "ablate",
            Self::Migrate => "migrate",
        }
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn file_hash(path: &Path) -> Result<String> {
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "Not a regular file: {}",
        display(path)
    );
    let mut hash = Sha256::new();
    io::copy(&mut File::open(path)?, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn current(path: &Path) -> Result<Option<String>> {
    if path.try_exists()? {
        Ok(Some(file_hash(path)?))
    } else {
        Ok(None)
    }
}

fn path_key(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        path.to_owned()
    }
}

pub(crate) fn safe_path(root: &Path, name: &str) -> Result<PathBuf> {
    ensure!(
        !name.contains(['\\', '\0', ':'])
            && name.split('/').all(|part| !part.is_empty()
                && !matches!(part, "." | "..")
                && !part.eq_ignore_ascii_case(".git")),
        "Invalid snapshot path: {name}"
    );
    #[cfg(windows)]
    for part in name.split('/') {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        ensure!(
            !part.ends_with(['.', ' '])
                && !matches!(
                    stem.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                )
                && !stem
                    .strip_prefix("COM")
                    .or_else(|| stem.strip_prefix("LPT"))
                    .is_some_and(|n| n.len() == 1 && "123456789".contains(n)),
            "Invalid snapshot path: {name}"
        );
    }
    let path = root.join(name);
    check_write(root, &path)?;
    Ok(path)
}

struct Store {
    root: PathBuf,
    base: PathBuf,
    dir: PathBuf,
    op: Operation,
}

impl Store {
    fn new(root: &Path, state: Option<&Path>, op: Operation) -> Result<Self> {
        let root = canonicalize(root)?;
        let base = match state {
            Some(path) => resolve(&std::env::current_dir()?, path)?,
            None => std::env::home_dir()
                .context("Cannot locate home directory; pass --state-dir")?
                .join(format!(".keel-{}", op.name())),
        };
        let mut anchor = base.as_path();
        while !anchor.try_exists()? {
            anchor = anchor.parent().context("Invalid snapshot directory")?;
        }
        for ancestor in base.ancestors() {
            #[cfg(target_os = "macos")]
            if matches!(ancestor.to_str(), Some("/var" | "/tmp")) {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(meta) => ensure!(
                    !linked_entry(&meta) && meta.is_dir(),
                    "Snapshot directory must not traverse a symbolic link or a file: {}",
                    display(ancestor)
                ),
                Err(err) if err.kind() == io::ErrorKind::NotFound => (),
                Err(err) => return Err(err.into()),
            }
        }
        let base = canonicalize(anchor)?.join(base.strip_prefix(anchor)?);
        let key = digest(display(&root).as_bytes());
        let dir = safe_path(&base, &format!("{}-{}", op.name(), &key[..16]))?;
        ensure!(
            !base.starts_with(&root) && !root.starts_with(&dir),
            "Snapshot directory must be outside the repository"
        );
        Ok(Self {
            root,
            base,
            dir,
            op,
        })
    }

    fn lock(&self) -> Result<File> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.base)?;
        }
        #[cfg(not(unix))]
        fs::create_dir_all(&self.base)?;
        let name = format!(
            "{}.lock",
            self.dir.file_name().unwrap_or_default().to_string_lossy()
        );
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(safe_path(&self.base, &name)?)?;
        lock.try_lock()
            .context("Another snapshot operation is active")?;
        check_write(&self.base, &self.dir)?;
        Ok(lock)
    }

    fn manifest(&self) -> Result<Manifest> {
        let path = safe_path(&self.dir, "manifest.json")?;
        let manifest: Manifest =
            serde_json::from_reader(BufReader::new(File::open(path).with_context(|| {
                format!(
                    "No active {} snapshot: {}",
                    self.op.name(),
                    display(&self.dir)
                )
            })?))?;
        ensure!(
            manifest.version == 1
                && manifest.operation == self.op
                && manifest.repo_root == display(&self.root),
            "Snapshot manifest does not match repository or operation"
        );
        Ok(manifest)
    }
}

struct Mutation {
    original: Option<String>,
    replacement: Option<Vec<u8>>,
}

#[derive(Default)]
pub(crate) struct Plan {
    files: BTreeMap<String, Mutation>,
    directories: BTreeSet<String>,
}

impl Plan {
    pub fn remove(&mut self, file: &str) {
        self.files.insert(
            file.into(),
            Mutation {
                original: None,
                replacement: None,
            },
        );
    }
    pub fn edit(&mut self, file: &str, original: &[u8], replacement: Option<Vec<u8>>) {
        self.files.insert(
            file.into(),
            Mutation {
                original: Some(digest(original)),
                replacement,
            },
        );
    }
    pub fn tree(&mut self, root: &Path, name: &str) -> Result<()> {
        let path = safe_path(root, name)?;
        if !path.try_exists()? {
            return Ok(());
        }
        ensure!(path.is_dir(), "Expected directory: {name}");
        for entry in WalkDir::new(path).follow_links(false) {
            let entry = entry?;
            let filename = entry
                .file_name()
                .to_str()
                .context("Cannot snapshot a non-UTF-8 filename without changing it")?;
            ensure!(
                !filename.contains('\\'),
                "Cannot represent a backslash in snapshot filename: {filename}"
            );
            let name = relative(root, entry.path());
            safe_path(root, &name)?;
            if entry.file_type().is_dir() {
                self.directories.insert(name);
            } else if entry.file_type().is_file() {
                self.remove(&name);
            } else {
                bail!("Unsupported entry in snapshot tree: {name}");
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    relative_path: String,
    original_sha256: String,
    post_sha256: Option<String>,
    mode: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    version: u8,
    operation: Operation,
    repo_root: String,
    created_at: String,
    entries: Vec<Entry>,
    directories: BTreeMap<String, u32>,
}

#[cfg(unix)]
fn mode(permissions: Permissions) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    permissions.mode() & 0o777
}
#[cfg(windows)]
fn mode(permissions: Permissions) -> u32 {
    u32::from(permissions.readonly())
}

fn permissions(source: &Path, mode: u32) -> Result<Permissions> {
    let mut perms = fs::metadata(source)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(mode & 0o777);
    }
    #[cfg(windows)]
    perms.set_readonly(mode != 0);
    Ok(perms)
}

fn verified_copy(
    source: &Path,
    dest: &Path,
    expected: &str,
    perms: Option<Permissions>,
    replace: bool,
) -> Result<()> {
    atomic_file(dest, perms, replace, |file| {
        io::copy(&mut File::open(source)?, file)?;
        use std::io::Seek;
        file.rewind()?;
        let mut hash = Sha256::new();
        io::copy(file, &mut hash)?;
        if format!("{:x}", hash.finalize()) != expected {
            return Err(io::Error::other(format!(
                "Backup integrity check failed: {}",
                display(source)
            )));
        }
        Ok(())
    })
}

fn backup_path(dir: &Path, entry: &Entry) -> Result<PathBuf> {
    safe_path(dir, &format!("files/{}", entry.relative_path))
}

pub(crate) fn apply(
    root: &Path,
    op: Operation,
    plan: Plan,
    state: Option<&Path>,
    dry_run: bool,
) -> Result<Value> {
    let store = Store::new(root, state, op)?;
    ensure!(
        !store.dir.try_exists()?,
        "An active {} snapshot already exists: {}",
        op.name(),
        display(&store.dir)
    );
    let mut entries = Vec::new();
    for (name, change) in &plan.files {
        let path = safe_path(root, name)?;
        let hash = file_hash(&path)?;
        ensure!(
            change.original.as_ref().is_none_or(|old| *old == hash),
            "File changed during planning: {name}"
        );
        entries.push(Entry {
            relative_path: name.clone(),
            original_sha256: hash,
            post_sha256: change.replacement.as_deref().map(digest),
            mode: mode(fs::metadata(path)?.permissions()),
        });
    }
    let mut directories = BTreeMap::new();
    for name in &plan.directories {
        directories.insert(
            name.clone(),
            mode(fs::metadata(safe_path(root, name)?)?.permissions()),
        );
    }
    ensure!(
        op != Operation::Ablate || !entries.is_empty() || !directories.is_empty(),
        "No Keel surfaces found to ablate"
    );
    let report = json!({"dryRun":dry_run,"snapshotDir":display(&store.dir),"entries":entries,"directories":plan.directories});
    if dry_run || (entries.is_empty() && directories.is_empty()) {
        return Ok(report);
    }
    let _lock = store.lock()?;
    ensure!(!store.dir.try_exists()?, "Snapshot already exists");
    let staging = tempfile::Builder::new()
        .prefix(".keel-staging-")
        .tempdir_in(&store.base)?;
    for entry in &entries {
        verified_copy(
            &safe_path(root, &entry.relative_path)?,
            &backup_path(staging.path(), entry)?,
            &entry.original_sha256,
            None,
            false,
        )?;
    }
    for entry in &entries {
        ensure!(
            file_hash(&safe_path(root, &entry.relative_path)?)? == entry.original_sha256,
            "File changed during backup: {}",
            entry.relative_path
        );
    }
    for name in directories.keys() {
        for child in fs::read_dir(safe_path(root, name)?)? {
            let child = relative(root, &child?.path());
            ensure!(
                plan.files.contains_key(&child) || directories.contains_key(&child),
                "Directory changed during backup: {name}"
            );
        }
    }
    let manifest = Manifest {
        version: 1,
        operation: op,
        repo_root: display(&store.root),
        created_at: Utc::now().to_rfc3339(),
        entries,
        directories,
    };
    crate::paths::write_bytes(
        &staging.path().join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest)?,
        false,
    )?;
    fs::rename(staging.path(), &store.dir)?;
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let commit = (|| -> Result<()> {
        for entry in &manifest.entries {
            let target = safe_path(root, &entry.relative_path)?;
            ensure!(
                file_hash(&target)? == entry.original_sha256,
                "File changed during cleanup: {}",
                entry.relative_path
            );
            if let Some(bytes) = &plan.files[&entry.relative_path].replacement {
                crate::paths::write_bytes(&target, bytes, true)?;
            } else {
                fs::remove_file(target)?;
            }
            changed.push(entry);
        }
        for name in manifest.directories.keys().rev() {
            fs::remove_dir(safe_path(root, name)?)?;
            removed.push(name);
        }
        Ok(())
    })();
    if let Err(err) = commit {
        let undo = (|| -> Result<()> {
            for name in removed.iter().rev() {
                fs::create_dir(safe_path(root, name)?)?;
            }
            for entry in changed {
                restore_entry(&store, entry, false)?;
            }
            for name in removed {
                let path = safe_path(root, name)?;
                fs::set_permissions(&path, permissions(&path, manifest.directories[name])?)?;
            }
            fs::remove_dir_all(&store.dir)?;
            Ok(())
        })();
        if let Err(undo) = undo {
            bail!(
                "{err}; rollback incomplete: {undo}. Verified snapshot retained at {}",
                display(&store.dir)
            );
        }
        return Err(err.context("Cleanup rolled back"));
    }
    Ok(report)
}

fn restore_entry(store: &Store, entry: &Entry, force: bool) -> Result<()> {
    let target = safe_path(&store.root, &entry.relative_path)?;
    let before = current(&target)?;
    ensure!(
        force || before == entry.post_sha256,
        "Restore conflict: {}",
        entry.relative_path
    );
    let backup = backup_path(&store.dir, entry)?;
    verified_copy(
        &backup,
        &target,
        &entry.original_sha256,
        Some(permissions(&backup, entry.mode)?),
        before.is_some(),
    )
}

pub(crate) fn restore(
    root: &Path,
    op: Operation,
    state: Option<&Path>,
    dry_run: bool,
    force: bool,
) -> Result<Value> {
    let store = Store::new(root, state, op)?;
    ensure!(
        store.dir.is_dir(),
        "No active {} snapshot: {}",
        op.name(),
        display(&store.dir)
    );
    let _lock = if dry_run { None } else { Some(store.lock()?) };
    let manifest = store.manifest()?;
    let mut conflicts = Vec::new();
    let mut names = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut prior = Vec::new();
    for name in manifest.directories.keys() {
        let dir = safe_path(root, name)?;
        if dir.try_exists()? {
            ensure!(dir.is_dir(), "Cannot restore directory over a file: {name}");
            conflicts.push(name);
        }
        ensure!(
            names.insert(path_key(&dir)),
            "Duplicate snapshot directory: {name}"
        );
    }
    for entry in &manifest.entries {
        let name = &entry.relative_path;
        let target = safe_path(root, name)?;
        ensure!(
            names.insert(path_key(&target)),
            "Duplicate snapshot path: {name}"
        );
        files.insert(path_key(&target));
        let hash = current(&target)?;
        if hash != entry.post_sha256 {
            conflicts.push(name);
        }
        prior.push(hash);
        ensure!(
            file_hash(&backup_path(&store.dir, entry)?)? == entry.original_sha256,
            "Snapshot integrity check failed for {name}"
        );
    }
    for path in &names {
        ensure!(
            !path
                .ancestors()
                .skip(1)
                .any(|parent| files.contains(parent)),
            "Conflicting manifest paths: {}",
            display(path)
        );
    }
    ensure!(
        force || conflicts.is_empty(),
        "Restore aborted: managed paths changed: {}. Use --force to overwrite",
        conflicts
            .into_iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    );
    if !dry_run {
        restore_transaction(&store, &manifest, &prior)?;
    }
    Ok(
        json!({"dryRun":dry_run,"restored":manifest.entries.iter().map(|entry| &entry.relative_path).collect::<Vec<_>>() }),
    )
}

fn restore_transaction(store: &Store, manifest: &Manifest, prior: &[Option<String>]) -> Result<()> {
    let undo = tempfile::Builder::new()
        .prefix(".keel-undo-")
        .tempdir_in(&store.base)?;
    let mut dirs = BTreeSet::new();
    let mut old_modes = BTreeMap::new();
    for name in manifest.directories.keys() {
        let path = safe_path(&store.root, name)?;
        if path.is_dir() {
            old_modes.insert(path.clone(), fs::metadata(&path)?.permissions());
        }
        dirs.insert(path);
    }
    for (entry, hash) in manifest.entries.iter().zip(prior) {
        let path = safe_path(&store.root, &entry.relative_path)?;
        if let Some(hash) = hash {
            let backup = backup_path(undo.path(), entry)?;
            verified_copy(&path, &backup, hash, None, false)?;
            old_modes.insert(path.clone(), fs::metadata(&path)?.permissions());
        }
        dirs.extend(
            path.ancestors()
                .skip(1)
                .take_while(|parent| *parent != store.root)
                .map(Path::to_owned),
        );
    }
    let mut created = Vec::new();
    let mut written = Vec::new();
    let mut chmods = Vec::new();
    let result = (|| -> Result<()> {
        for dir in dirs {
            check_write(&store.root, &dir)?;
            if !dir.try_exists()? {
                fs::create_dir(&dir)?;
                created.push(dir);
            }
        }
        for (index, entry) in manifest.entries.iter().enumerate() {
            ensure!(
                current(&safe_path(&store.root, &entry.relative_path)?)? == prior[index],
                "File changed during restore: {}",
                entry.relative_path
            );
            restore_entry(store, entry, true)?;
            written.push(index);
        }
        for (name, mode) in manifest.directories.iter().rev() {
            let dir = safe_path(&store.root, name)?;
            fs::set_permissions(&dir, permissions(&dir, *mode)?)?;
            chmods.push(dir);
        }
        Ok(())
    })();
    if let Err(error) = result {
        let rollback = (|| -> Result<()> {
            for dir in chmods.iter().rev() {
                if let Some(mode) = old_modes.get(dir) {
                    fs::set_permissions(dir, mode.clone())?;
                }
                #[cfg(unix)]
                if !old_modes.contains_key(dir) {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(dir, Permissions::from_mode(0o700))?;
                }
            }
            for index in written.into_iter().rev() {
                let entry = &manifest.entries[index];
                let path = safe_path(&store.root, &entry.relative_path)?;
                ensure!(
                    file_hash(&path)? == entry.original_sha256,
                    "Changed during rollback: {}",
                    entry.relative_path
                );
                if let Some(hash) = &prior[index] {
                    verified_copy(
                        &backup_path(undo.path(), entry)?,
                        &path,
                        hash,
                        old_modes.get(&path).cloned(),
                        true,
                    )?;
                } else {
                    fs::remove_file(path)?;
                }
            }
            for dir in created.into_iter().rev() {
                check_write(&store.root, &dir)?;
                fs::remove_dir(dir)?;
            }
            Ok(())
        })();
        if let Err(secondary) = rollback {
            bail!(
                "{error}; restore rollback incomplete: {secondary}; recovery copies retained at {}",
                display(&undo.keep())
            );
        }
        return Err(error.context("Restore rolled back; snapshot retained"));
    }
    fs::remove_dir_all(&store.dir)?;
    Ok(())
}

pub(crate) fn status(root: &Path, state: Option<&Path>) -> Result<Value> {
    let store = Store::new(root, state, Operation::Ablate)?;
    let mut result = json!({"ablated":false,"snapshotDir":display(&store.dir),"entryCount":0});
    if store.dir.try_exists()? {
        let manifest = store.manifest()?;
        result["ablated"] = true.into();
        result["createdAt"] = manifest.created_at.into();
        result["entryCount"] = manifest.entries.len().into();
    }
    Ok(result)
}
