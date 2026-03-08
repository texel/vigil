use anyhow::Result;
use vigil_registry::Store;
use vigil_scheduler_launchd::LaunchdScheduler;

pub async fn handle(name: &str, store: &Store) -> Result<()> {
    let task = store
        .get_task_by_name(name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task '{name}' not found"))?;

    // Unregister from launchd
    let scheduler = LaunchdScheduler::new()?;
    vigil_core::scheduler::Scheduler::unregister(&scheduler, name).await?;

    // Clear trigger in DB
    store.update_trigger(task.id, None).await?;

    println!("Unscheduled '{name}'");
    Ok(())
}
