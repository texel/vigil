use anyhow::Result;
use chrono::Utc;
use vigil_core::models::RunStatus;
use vigil_registry::Store;

use crate::format::format_duration;

pub async fn handle(name: Option<&str>, limit: u32, verbose: bool, store: &Store) -> Result<()> {
    let runs = store.list_recent_runs(name, limit).await?;

    if runs.is_empty() {
        println!("No runs found.");
        return Ok(());
    }

    if verbose {
        println!(
            "{:<38} {:<14} {:<10} {:<21} {:<10} {:#}",
            "RUN ID", "TASK", "STATUS", "STARTED", "DURATION", "EXIT"
        );
        println!("{}", "\u{2500}".repeat(103));
    } else {
        println!(
            "{:<10} {:<14} {:<10} {:<21} {:<10} {:#}",
            "RUN ID", "TASK", "STATUS", "STARTED", "DURATION", "EXIT"
        );
        println!("{}", "\u{2500}".repeat(75));
    }

    for (run, task_name) in &runs {
        let id_str = if verbose {
            run.id.to_string()
        } else {
            run.id.to_string()[..8].to_string()
        };
        let status = run.status.to_string();
        let started = run.started_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let duration = match run.completed_at {
            Some(completed) => {
                let dur = completed - run.started_at;
                format_duration(dur)
            }
            None => match run.status {
                RunStatus::Running => {
                    let dur = Utc::now() - run.started_at;
                    format!("{}...", format_duration(dur))
                }
                _ => "-".to_string(),
            },
        };
        let exit = run
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string());

        if verbose {
            println!(
                "{:<38} {:<14} {:<10} {:<21} {:<10} {}",
                id_str, task_name, status, started, duration, exit
            );
        } else {
            println!(
                "{:<10} {:<14} {:<10} {:<21} {:<10} {}",
                id_str, task_name, status, started, duration, exit
            );
        }
    }

    println!("\n{} run(s)", runs.len());
    Ok(())
}
