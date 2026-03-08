use anyhow::Result;
use vigil_registry::Store;
use vigil_scheduler_launchd::LaunchdScheduler;

pub async fn handle(store: &Store) -> Result<()> {
    let tasks = store.list_tasks().await?;

    if tasks.is_empty() {
        println!("No tasks registered.");
        return Ok(());
    }

    let scheduler = LaunchdScheduler::new()?;

    println!(
        "{:<20} {:<10} {:<25} {:<12}",
        "NAME", "RUNNER", "SCHEDULE", "LAUNCHD"
    );
    println!("{}", "-".repeat(67));

    for task in &tasks {
        let schedule_str = task
            .trigger
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "-".to_string());

        let launchd_status =
            vigil_core::scheduler::Scheduler::status(&scheduler, &task.name).await?;

        println!(
            "{:<20} {:<10} {:<25} {:<12}",
            task.name, task.task.runner_type, schedule_str, launchd_status
        );
    }

    println!("\n{} task(s)", tasks.len());
    Ok(())
}
