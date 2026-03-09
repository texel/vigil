use anyhow::Result;
use vigil_registry::Store;
use vigil_schedule::TriggerSpec;
use vigil_scheduler_launchd::LaunchdScheduler;

pub async fn handle(name: &str, trigger_words: &[String], store: &Store) -> Result<()> {
    let trigger_expr = trigger_words.join(" ");
    let trigger: TriggerSpec = trigger_expr.parse()?;

    let task = store
        .get_task_by_name(name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task '{name}' not found"))?;

    // Update trigger in DB
    store.update_trigger(task.id, Some(&trigger)).await?;

    // Register with launchd
    let scheduler = LaunchdScheduler::new()?;
    vigil_core::scheduler::Scheduler::register(&scheduler, name, &trigger).await?;

    println!("Scheduled '{name}': {trigger}");
    Ok(())
}
