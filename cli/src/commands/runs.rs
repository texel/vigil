use anyhow::Result;
use chrono::Utc;
use vigil_core::models::RunStatus;
use vigil_registry::Store;

pub async fn handle(name: Option<&str>, limit: u32, store: &Store) -> Result<()> {
    let runs = store.list_recent_runs(name, limit).await?;

    if runs.is_empty() {
        println!("No runs found.");
        return Ok(());
    }

    println!(
        "{:<10} {:<14} {:<10} {:<21} {:<10} {:#}",
        "RUN ID", "TASK", "STATUS", "STARTED", "DURATION", "EXIT"
    );
    println!("{}", "\u{2500}".repeat(75));

    for (run, task_name) in &runs {
        let short_id = &run.id.to_string()[..8];
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

        println!(
            "{:<10} {:<14} {:<10} {:<21} {:<10} {}",
            short_id, task_name, status, started, duration, exit
        );
    }

    println!("\n{} run(s)", runs.len());
    Ok(())
}

fn format_duration(dur: chrono::TimeDelta) -> String {
    let total_secs = dur.num_milliseconds() as f64 / 1000.0;
    if total_secs < 60.0 {
        format!("{:.1}s", total_secs)
    } else if total_secs < 3600.0 {
        format!(
            "{:.0}m {:.0}s",
            (total_secs / 60.0).floor(),
            total_secs % 60.0
        )
    } else {
        let hours = (total_secs / 3600.0).floor();
        let mins = ((total_secs % 3600.0) / 60.0).floor();
        format!("{:.0}h {:.0}m", hours, mins)
    }
}
