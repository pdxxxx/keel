use crate::paths::posix;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) fn changed_files(root: &Path) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for args in [
        &["diff", "--name-only", "-z"][..],
        &["diff", "--cached", "--name-only", "-z"],
        &["ls-files", "--others", "--exclude-standard", "-z"],
    ] {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .stdin(Stdio::null())
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                paths.extend(
                    output
                        .stdout
                        .split(|&byte| byte == 0)
                        .filter(|name| !name.is_empty())
                        .map(|name| posix(&String::from_utf8_lossy(name))),
                );
            }
        }
    }
    paths
}
