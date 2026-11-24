use std::fs::{self, Permissions};
use std::io;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

#[cfg(windows)]
pub(crate) use dunce::canonicalize;
#[cfg(not(windows))]
pub(crate) use std::fs::canonicalize;

pub(crate) fn atomic_file(
    path: &Path,
    permissions: Option<Permissions>,
    replace: bool,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing parent: {}", display(path)))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    write(temp.as_file_mut())?;
    if let Some(mode) = permissions {
        temp.as_file().set_permissions(mode)?;
    }
    temp.as_file().sync_all()?;
    if replace {
        temp.persist(path)?;
    } else {
        temp.persist_noclobber(path)?;
    }
    Ok(())
}

pub(crate) fn write_bytes(path: &Path, bytes: &[u8], replace: bool) -> anyhow::Result<()> {
    let permissions = if replace {
        Some(fs::metadata(path)?.permissions())
    } else {
        None
    };
    atomic_file(path, permissions, replace, |file| file.write_all(bytes))
}

pub fn resolve(base: &Path, path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(base.join(path))?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

pub fn discover(start: &Path) -> io::Result<PathBuf> {
    let start = resolve(start, Path::new(""))?;
    Ok(find_repo(&start).unwrap_or(&start).to_owned())
}

pub(crate) fn find_repo(start: &Path) -> Option<&Path> {
    let mut git = None;
    for directory in start.ancestors() {
        if directory.join(".keel").is_dir() {
            return Some(directory);
        }
        if git.is_none() && directory.join(".git").exists() {
            git = Some(directory);
        }
    }
    git
}

pub(crate) fn posix(path: &str) -> String {
    let path = path.replace('\\', "/");
    path.strip_prefix("./").unwrap_or(&path).to_owned()
}

pub(crate) fn display(path: &Path) -> String {
    posix(&path.to_string_lossy())
}

pub(crate) fn relative(root: &Path, path: &Path) -> String {
    display(path.strip_prefix(root).unwrap_or(path))
}

pub(crate) fn linked_entry(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.is_symlink()
    }
}

pub(crate) fn check_write(root: &Path, target: &Path) -> anyhow::Result<()> {
    let mut current = root.to_owned();
    for part in target.strip_prefix(root)?.components() {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if linked_entry(&meta) => anyhow::bail!(
                "Refusing symbolic link in Keel write path: {}",
                relative(root, &current)
            ),
            Ok(meta) if current != target && !meta.is_dir() => {
                anyhow::bail!("Not a directory: {}", relative(root, &current))
            }
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err.into()),
            _ => (),
        }
    }
    Ok(())
}

pub(crate) fn read_text(path: &Path) -> anyhow::Result<String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(error) => {
            let path = display(path);
            match error.kind() {
                io::ErrorKind::IsADirectory => {
                    anyhow::bail!("EISDIR: illegal operation on a directory, read")
                }
                io::ErrorKind::NotFound => {
                    anyhow::bail!("ENOENT: no such file or directory, open '{path}'")
                }
                io::ErrorKind::PermissionDenied => {
                    anyhow::bail!("EACCES: permission denied, open '{path}'")
                }
                _ => Err(error.into()),
            }
        }
    }
}

pub(crate) fn js_whitespace(character: char) -> bool {
    character == '\u{feff}' || (character.is_whitespace() && character != '\u{85}')
}

pub(crate) fn trim(text: &str) -> &str {
    text.trim_matches(js_whitespace)
}

pub(crate) fn ascii_slug(text: &str, preserved: &str) -> String {
    let mut result = String::new();
    let mut replacing = false;
    for character in text.to_lowercase().chars() {
        if character.is_ascii_alphanumeric() || preserved.contains(character) {
            result.push(character);
            replacing = false;
        } else if !replacing {
            result.push('-');
            replacing = true;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn keel_ancestor_precedes_nearer_git_file() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("worktree/deep");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(discover(&nested).unwrap(), nested);
        fs::write(nested.parent().unwrap().join(".git"), "gitdir: elsewhere").unwrap();
        assert_eq!(discover(&nested).unwrap(), nested.parent().unwrap());
        fs::create_dir(temp.path().join(".keel")).unwrap();
        assert_eq!(discover(&nested).unwrap(), temp.path());
        assert_eq!(
            resolve(&nested, Path::new(".././file")).unwrap(),
            temp.path().join("worktree/file")
        );
    }
}
