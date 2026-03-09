use anyhow::{Result, bail};
use chrono::Utc;
use vigil_core::models::RunStatus;
use vigil_registry::Store;
use vigil_scheduler_launchd::LaunchdScheduler;

use crate::format::format_duration;

pub async fn handle(name_or_id: &str, store: &Store) -> Result<()> {
    // Resolve task: try name first, then ID prefix
    let task = match store.get_task_by_name(name_or_id).await? {
        Some(t) => t,
        None => match store.get_task_by_id_prefix(name_or_id).await? {
            Some(t) => t,
            None => bail!("no task found matching '{name_or_id}'"),
        },
    };

    // Task info section
    println!("Task:       {}", task.name);
    println!("ID:         {}", task.id);
    println!("Runner:     {}", task.task.runner_type);
    if let Some(dir) = &task.working_directory {
        println!("Directory:  {}", dir.display());
    }
    println!(
        "Enabled:    {}",
        if task.enabled { "yes" } else { "no" }
    );
    println!(
        "Created:    {}",
        task.created_at.format("%Y-%m-%d %H:%M:%S")
    );

    // Schedule section
    println!();
    match &task.trigger {
        Some(trigger) => {
            println!("Schedule:   {trigger}");

            let scheduler = LaunchdScheduler::new()?;
            let launchd_status =
                vigil_core::scheduler::Scheduler::status(&scheduler, &task.name).await?;
            println!("Launchd:    {launchd_status}");

            let now = Utc::now();
            match trigger.next_occurrence(now) {
                Some(next) => println!("Next run:   {}", next.format("%Y-%m-%d %H:%M:%S UTC")),
                None => println!("Next run:   -"),
            }
        }
        None => {
            println!("Schedule:   not scheduled");
        }
    }

    // Recent runs section
    let runs = store.get_runs_for_task(task.id, 5).await?;
    println!();
    if runs.is_empty() {
        println!("No runs yet.");
    } else {
        println!("Recent runs ({}):", runs.len());
        println!(
            "{:<10} {:<10} {:<21} {:<10} {}",
            "RUN ID", "STATUS", "STARTED", "DURATION", "EXIT"
        );
        println!("{}", "\u{2500}".repeat(61));

        for run in &runs {
            let id_str = &run.id.to_string()[..8];
            let status = run.status.to_string();
            let started = run.started_at.format("%Y-%m-%d %H:%M:%S").to_string();
            let duration = match run.completed_at {
                Some(completed) => format_duration(completed - run.started_at),
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

            println!("{:<10} {:<10} {:<21} {:<10} {}", id_str, status, started, duration, exit);
        }
    }

    Ok(())
}
