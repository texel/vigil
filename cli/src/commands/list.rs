use anyhow::Result;
use vigil_registry::Store;

pub async fn handle(store: &Store) -> Result<()> {
    let tasks = store.list_tasks().await?;

    if tasks.is_empty() {
        println!("No tasks registered.");
        return Ok(());
    }

    println!(
        "{:<20} {:<10} {:<8} {:#}",
        "NAME", "RUNNER", "ENABLED", "WORKING DIR"
    );
    println!("{}", "-".repeat(70));

    for task in &tasks {
        let dir = task
            .working_directory
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "-".to_string());
        let enabled = if task.enabled { "yes" } else { "no" };
        println!(
            "{:<20} {:<10} {:<8} {}",
            task.name, task.task.runner_type, enabled, dir
        );
    }

    println!("\n{} task(s)", tasks.len());
    Ok(())
}
