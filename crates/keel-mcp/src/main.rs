use anyhow::{Result, bail};
use keel_mcp::{mcp, paths};
use std::env;
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::ExitCode;

fn run() -> Result<()> {
    let mut root = None;
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                eprintln!(
                    "keel-mcp {}\nUsage: keel-mcp [--root <path>]",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            Some("--version" | "-V") => {
                eprintln!("keel-mcp {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Some("--root") if root.is_none() => {
                let Some(path) = args.next().filter(|value| !value.is_empty()) else {
                    bail!("--root requires a path");
                };
                root = Some(PathBuf::from(path));
            }
            _ => bail!(
                "Unexpected argument: {} (usage: keel-mcp [--root <path>])",
                arg.to_string_lossy()
            ),
        }
    }
    let cwd = env::current_dir()?;
    let root = match root {
        Some(root) => paths::resolve(&cwd, &root)?,
        None => paths::discover(&cwd)?,
    };
    let result = mcp::serve(
        &root,
        BufReader::new(io::stdin().lock()),
        BufWriter::new(io::stdout().lock()),
    );
    match result {
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => Ok(other?),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("keel-mcp: {err:#}");
            ExitCode::FAILURE
        }
    }
}
