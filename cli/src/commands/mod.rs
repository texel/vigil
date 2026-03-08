mod claude;
mod list;
mod register;
mod run;
mod runs;
mod schedule;
mod status;
mod unregister;
mod unschedule;

use anyhow::Result;
use clap::{Parser, Subcommand};
use vigil_registry::Store;

#[derive(Parser)]
#[command(name = "vigil", about = "Task scheduler for Claude Code and beyond")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Register a new task
    Register {
        #[command(subcommand)]
        executor: RegisterExecutor,
    },
    /// Run a task now
    Run {
        /// Task name
        name: String,
        /// Suppress streaming output
        #[arg(short, long)]
        quiet: bool,
        /// Preview what would run without executing
        #[arg(long)]
        dry_run: bool,
    },
    /// List all registered tasks
    List,
    /// Remove a registered task
    Unregister {
        /// Task name
        name: String,
    },
    /// Claude-specific commands
    Claude {
        #[command(subcommand)]
        command: ClaudeCommand,
    },
    /// Resume a failed Claude session interactively (shortcut for `claude resume`)
    Resume {
        /// Run ID (full UUID or short prefix)
        run_id: String,
    },
    /// Schedule a task for recurring execution
    Schedule {
        /// Task name
        name: String,
        /// Trigger expression (e.g. "daily at 09:00", "every 5 minutes")
        trigger: Vec<String>,
    },
    /// Remove a task's schedule
    Unschedule {
        /// Task name
        name: String,
    },
    /// Show task status and schedules
    Status,
    /// Show recent task runs
    Runs {
        /// Filter by task name
        name: Option<String>,
        /// Max runs to show
        #[arg(short, long, default_value = "20")]
        limit: u32,
        /// Show full UUIDs instead of short IDs
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
pub enum RegisterExecutor {
    /// Register a shell command as a task
    Shell {
        /// Task name
        name: String,
        /// Shell command to execute
        command: String,
        /// Working directory (defaults to current directory)
        #[arg(short, long)]
        dir: Option<String>,
    },
    /// Register a Claude Code task (skill or inline prompt)
    Claude {
        /// Task name (or skill shorthand like /daily-briefing)
        name: String,
        /// Skill name (e.g. /daily-briefing) or inline prompt
        prompt_or_skill: Option<String>,
        /// Working directory (defaults to current directory)
        #[arg(short, long)]
        dir: Option<String>,
        /// Allowed tools (comma-separated)
        #[arg(long)]
        allowed_tools: Option<String>,
        /// Max conversation turns
        #[arg(long)]
        max_turns: Option<u32>,
        /// Model to use
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ClaudeCommand {
    /// Resume a failed Claude session interactively
    Resume {
        /// Run ID (full UUID or short prefix)
        run_id: String,
    },
}

pub async fn dispatch(cli: Cli, store: Store) -> Result<()> {
    match cli.command {
        Command::Register { executor } => register::handle(executor, &store).await,
        Command::Run { name, quiet, dry_run } => run::handle(&name, quiet, dry_run, &store).await,
        Command::List => list::handle(&store).await,
        Command::Unregister { name } => unregister::handle(&name, &store).await,
        Command::Schedule { name, trigger } => schedule::handle(&name, &trigger, &store).await,
        Command::Unschedule { name } => unschedule::handle(&name, &store).await,
        Command::Status => status::handle(&store).await,
        Command::Claude { command } => match command {
            ClaudeCommand::Resume { run_id } => claude::resume::handle(&run_id, &store).await,
        },
        Command::Resume { run_id } => claude::resume::handle(&run_id, &store).await,
        Command::Runs { name, limit, verbose } => runs::handle(name.as_deref(), limit, verbose, &store).await,
    }
}
