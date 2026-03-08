use anyhow::Result;
use vigil_registry::Store;
use vigil_scheduler_launchd::LaunchdScheduler;

pub async fn handle(name: &str, store: &Store) -> Result<()> {
    let task = store
        .get_task_by_name(name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task '{name}' not found"))?;

    // Unschedule from launchd if task has a trigger
    if task.trigger.is_some() {
        let scheduler = LaunchdScheduler::new()?;
        vigil_core::scheduler::Scheduler::unregister(&scheduler, name).await?;
        println!("Unscheduled '{name}' from launchd");
    }

    store.delete_task(task.id).await?;
    println!("Unregistered task '{name}'");
    Ok(())
}
