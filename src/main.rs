mod ablate;
mod cli;
mod config;
mod doctor;
mod git;
mod init;
mod mcp;
mod mcp_root;
mod mem;
mod migrate;
mod paths;
mod rules;
mod snapshot;
mod tasks;
mod verify;

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|err| err.kind() == io::ErrorKind::BrokenPipe) =>
        {
            ExitCode::SUCCESS
        }
        Err(error) => {
            let message = format!("{error:#}");
            eprintln!(
                "keel: {}",
                message.split_whitespace().collect::<Vec<_>>().join(" ")
            );
            ExitCode::FAILURE
        }
    }
}
