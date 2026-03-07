use anyhow::Result;
use vigil_registry::Store;

pub async fn handle(name: &str, store: &Store) -> Result<()> {
    let task = store
        .get_task_by_name(name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task '{name}' not found"))?;

    store.delete_task(task.id).await?;
    println!("Unregistered task '{name}'");
    Ok(())
}
