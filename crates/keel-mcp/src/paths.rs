use std::io;
use std::path::{Component, Path, PathBuf};

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
    for marker in [".keel", ".git"] {
        if let Some(root) = start.ancestors().find(|dir| dir.join(marker).exists()) {
            return Ok(root.to_owned());
        }
    }
    Ok(start)
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
