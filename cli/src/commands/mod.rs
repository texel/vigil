mod register;
mod resume;
mod run;
mod runs;
mod list;
mod unregister;

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
    },
    /// List all registered tasks
    List,
    /// Remove a registered task
    Unregister {
        /// Task name
        name: String,
    },
    /// Resume a failed Claude session interactively
    Resume {
        /// Run ID (full UUID)
        run_id: String,
    },
    /// Show recent task runs
    Runs {
        /// Filter by task name
        name: Option<String>,
        /// Max runs to show
        #[arg(short, long, default_value = "20")]
        limit: u32,
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
        /// Task name
        name: String,
        /// Skill name (e.g. /daily-briefing) or inline prompt
        prompt_or_skill: String,
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

pub async fn dispatch(cli: Cli, store: Store) -> Result<()> {
    match cli.command {
        Command::Register { executor } => register::handle(executor, &store).await,
        Command::Run { name } => run::handle(&name, &store).await,
        Command::List => list::handle(&store).await,
        Command::Unregister { name } => unregister::handle(&name, &store).await,
        Command::Resume { run_id } => resume::handle(&run_id, &store).await,
        Command::Runs { name, limit } => runs::handle(name.as_deref(), limit, &store).await,
    }
}
