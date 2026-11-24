use crate::{
    ablate, doctor, git, init, mcp, mem, migrate, paths, rules,
    snapshot::{self, Operation},
    tasks, verify,
};
use anyhow::{Result, bail, ensure};
use clap::{Args, Parser, Subcommand, error::ErrorKind};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    env,
    io::{self, BufReader, BufWriter, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about = "Repository rules, task contracts, memory and verification",
    disable_help_subcommand = true
)]
struct Cli {
    #[arg(
        short = 'C',
        long,
        global = true,
        value_name = "PATH",
        help = "Target repository or a directory inside it"
    )]
    root: Option<PathBuf>,
    #[arg(long, global = true, help = "Print machine-readable JSON")]
    json: bool,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    #[command(about = "Initialize Keel without replacing user configuration")]
    Init {
        #[arg(short, long)]
        force: bool,
        #[arg(long)]
        reset_config: bool,
        #[arg(long)]
        no_skills: bool,
        #[arg(long)]
        no_detect: bool,
    },
    #[command(about = "Refresh managed templates, leaving config, tasks and rules alone")]
    Update {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_skills: bool,
    },
    #[command(about = "Check configuration, managed files, git and rule health")]
    Doctor,
    #[command(subcommand, about = "List, match, author and check engineering rules")]
    Rules(Rules),
    #[command(subcommand, about = "Create and maintain task contracts")]
    Task(Task),
    #[command(about = "Run deterministic verification gates")]
    Verify {
        #[arg(short='g',long="gate",num_args=1..)]
        gates: Vec<String>,
        #[command(flatten)]
        selection: Selection,
        #[arg(long)]
        fail_fast: bool,
    },
    #[command(about = "Rank existing files by recent Git bug-fix activity")]
    Hotspots {
        #[arg(long,default_value_t=100,value_parser=positive)]
        commits: usize,
        #[arg(long,default_value_t=10,value_parser=positive)]
        limit: usize,
    },
    #[command(subcommand, about = "Read local project and session memory")]
    Mem(Memory),
    #[command(
        subcommand,
        about = "Inspect and safely retire conflicting workflow files"
    )]
    Migrate(Migration),
    #[command(about = "Back up and remove Keel surfaces, or inspect the snapshot")]
    Ablate {
        #[command(subcommand)]
        command: Option<Ablate>,
        #[command(flatten)]
        backup: Backup,
    },
    #[command(about = "Restore a verified ablation snapshot")]
    Restore(Recovery),
    #[command(about = "Serve 13 tools over JSON-RPC stdio")]
    Mcp {
        #[arg(long, help = "Reject tool roots outside the startup directory")]
        lock_root: bool,
    },
}

#[derive(Args)]
#[group(multiple = false)]
struct Selection {
    #[arg(long)]
    git: bool,
    #[arg(long)]
    staged: bool,
    #[arg(long, value_name = "REF")]
    base: Option<String>,
}
impl Selection {
    fn mode(&self) -> Option<git::Changes<'_>> {
        if self.git {
            Some(git::Changes::Working)
        } else if self.staged {
            Some(git::Changes::Staged)
        } else {
            self.base.as_deref().map(git::Changes::Base)
        }
    }
}

#[derive(Subcommand)]
enum Rules {
    List,
    Match {
        paths: Vec<String>,
        #[command(flatten)]
        selection: Selection,
        #[arg(long)]
        no_global: bool,
    },
    Add {
        id: String,
        #[arg(short, long)]
        summary: String,
        #[arg(long,value_parser=["invariant","guideline","adr"])]
        severity: Option<String>,
        #[arg(short,long,num_args=1..)]
        paths: Vec<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(short, long)]
        force: bool,
    },
    Check {
        #[arg(long)]
        stale: bool,
        #[arg(long)]
        strict: bool,
    },
}
#[derive(Subcommand)]
enum Task {
    Create {
        title: String,
        #[arg(long)]
        slug: Option<String>,
    },
    List {
        #[arg(long)]
        all: bool,
        #[arg(long,value_parser=["open","done"])]
        status: Option<String>,
    },
    Show {
        id: String,
    },
    Done {
        id: String,
    },
    Reopen {
        id: String,
    },
    Archive {
        id: String,
    },
}

#[derive(Args)]
struct Filter {
    #[arg(long)]
    source: Option<String>,
    #[arg(long,default_value="all",value_parser=["all","project"])]
    scope: String,
}
#[derive(Subcommand)]
enum Memory {
    Sources {
        #[arg(long)]
        source: Option<String>,
    },
    Sessions {
        #[command(flatten)]
        filter: Filter,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    Search {
        query: String,
        #[command(flatten)]
        filter: Filter,
        #[arg(short = 'n', long, default_value_t = 15)]
        limit: usize,
    },
    Show {
        session_id: String,
        #[arg(short = 'n', long, default_value_t = 30)]
        limit: usize,
    },
}

#[derive(Args)]
struct Backup {
    #[arg(long)]
    dry_run: bool,
    #[arg(long, value_name = "DIR")]
    state_dir: Option<PathBuf>,
}
#[derive(Args)]
struct Recovery {
    #[command(flatten)]
    backup: Backup,
    #[arg(short, long)]
    force: bool,
}
#[derive(Subcommand)]
enum Migration {
    Scan,
    Plan,
    Cleanup(Backup),
    Rollback(Recovery),
}
#[derive(Subcommand)]
enum Ablate {
    Status {
        #[arg(long, value_name = "DIR")]
        state_dir: Option<PathBuf>,
    },
}

fn positive(value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| "expected a positive integer".to_owned())
}

fn refs(names: &[String]) -> Vec<&str> {
    names.iter().map(String::as_str).collect()
}

impl Cli {
    fn print(&self, value: &impl Serialize) -> Result<()> {
        let mut stdout = io::stdout().lock();
        if self.json {
            serde_json::to_writer(&mut stdout, value)?;
        } else {
            write!(stdout, "{}", yaml_serde::to_string(value)?.trim_end())?;
        }
        writeln!(stdout)?;
        Ok(())
    }
}

pub(crate) fn run() -> Result<()> {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            err.print()?;
            return Ok(());
        }
        Err(err) => bail!(
            "{}",
            err.to_string()
                .lines()
                .next()
                .unwrap_or("Invalid command")
                .trim_start_matches("error: ")
        ),
    };
    let cwd = env::current_dir()?;
    if let Action::Mcp { lock_root } = &cli.command {
        let roots = mcp::McpRoots::new(&cwd, cli.root.as_deref(), *lock_root)?;
        mcp::serve_session(
            roots,
            BufReader::new(io::stdin().lock()),
            BufWriter::new(io::stdout().lock()),
        )?;
        return Ok(());
    }
    let base = cli
        .root
        .as_ref()
        .map_or_else(|| cwd.clone(), |path| cwd.join(path));
    ensure!(
        base.is_dir(),
        "Invalid root directory: {}",
        paths::display(&base)
    );
    let root = &paths::discover(&paths::canonicalize(&base)?)?;
    match &cli.command {
        Action::Init {
            force,
            reset_config,
            no_skills,
            no_detect,
        } => cli.print(&init::setup(
            root,
            init::Setup::Init {
                force: *force,
                reset: *reset_config,
                detect: !*no_detect,
            },
            *no_skills,
        )?),
        Action::Update { dry_run, no_skills } => cli.print(&init::setup(
            root,
            init::Setup::Update { dry_run: *dry_run },
            *no_skills,
        )?),
        Action::Doctor => {
            let report = doctor::check(root);
            cli.print(&report)?;
            ensure!(
                report.errors.is_empty(),
                "Doctor found {} error(s): {}",
                report.errors.len(),
                report.errors.join("; ")
            );
            Ok(())
        }
        Action::Rules(action) => match action {
            Rules::List => cli.print(&rules::list(root)?),
            Rules::Match {
                paths,
                selection,
                no_global,
            } => {
                let mut targets: BTreeSet<_> =
                    paths.iter().map(|p| crate::paths::posix(p)).collect();
                if let Some(mode) = selection.mode() {
                    targets.extend(git::changed_files(root, mode)?);
                }
                let targets: Vec<_> = targets.into_iter().collect();
                let matched = rules::matching(root, &targets, !no_global)?;
                cli.print(&serde_json::json!({"targets":targets,"matched":matched}))
            }
            Rules::Add {
                id,
                summary,
                severity,
                paths,
                body,
                force,
            } => cli.print(&rules::add(
                root,
                id,
                summary,
                severity.as_deref(),
                &refs(paths),
                body.as_deref(),
                *force,
            )?),
            Rules::Check { stale, strict } => {
                let report = rules::check(root, *stale || *strict)?;
                cli.print(&report)?;
                ensure!(
                    report.valid && (!strict || report.warnings.as_ref().is_none_or(Vec::is_empty)),
                    "Rules check failed"
                );
                Ok(())
            }
        },
        Action::Task(task) => match task {
            Task::Create { title, slug } => {
                cli.print(&tasks::create(root, title, slug.as_deref())?)
            }
            Task::List { all, status } => cli.print(&tasks::list(root, status.as_deref(), *all)?),
            Task::Show { id } => cli.print(&tasks::show(root, id)?),
            Task::Done { id } => cli.print(&tasks::update(root, id, "done")?),
            Task::Reopen { id } => cli.print(&tasks::update(root, id, "reopen")?),
            Task::Archive { id } => cli.print(&tasks::update(root, id, "archive")?),
        },
        Action::Verify {
            gates,
            selection,
            fail_fast,
        } => {
            let report = verify::run(root, &refs(gates), *fail_fast, selection.mode())?;
            cli.print(&report)?;
            ensure!(report.passed, "Verification failed");
            Ok(())
        }
        Action::Hotspots { commits, limit } => cli.print(&git::hotspots(root, *commits, *limit)),
        Action::Mem(action) => match action {
            Memory::Sources { source } => cli.print(&mem::sources(root, source.as_deref())?),
            Memory::Sessions { filter, limit } => cli.print(&mem::sessions(
                root,
                filter.source.as_deref(),
                *limit,
                filter.scope == "project",
            )?),
            Memory::Search {
                query,
                filter,
                limit,
            } => cli.print(&mem::search(
                root,
                query,
                filter.source.as_deref(),
                *limit,
                filter.scope == "project",
            )?),
            Memory::Show { session_id, limit } => cli.print(&mem::show(root, session_id, *limit)?),
        },
        Action::Migrate(action) => match action {
            Migration::Scan => cli.print(&migrate::scan(root)?),
            Migration::Plan => cli.print(&migrate::plan(root)?),
            Migration::Cleanup(opts) => cli.print(&migrate::cleanup(
                root,
                opts.state_dir.as_deref(),
                opts.dry_run,
            )?),
            Migration::Rollback(opts) => cli.print(&snapshot::restore(
                root,
                Operation::Migrate,
                opts.backup.state_dir.as_deref(),
                opts.backup.dry_run,
                opts.force,
            )?),
        },
        Action::Ablate {
            command: None,
            backup,
        } => cli.print(&ablate::run(
            root,
            backup.state_dir.as_deref(),
            backup.dry_run,
        )?),
        Action::Ablate {
            command: Some(Ablate::Status { state_dir }),
            backup,
        } => cli.print(&snapshot::status(
            root,
            state_dir.as_deref().or(backup.state_dir.as_deref()),
        )?),
        Action::Restore(opts) => cli.print(&snapshot::restore(
            root,
            Operation::Ablate,
            opts.backup.state_dir.as_deref(),
            opts.backup.dry_run,
            opts.force,
        )?),
        Action::Mcp { .. } => Ok(()),
    }
}
