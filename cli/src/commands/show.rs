use anyhow::Result;
use vigil_registry::Store;

pub async fn handle(run_id: &str, raw: bool, store: &Store) -> Result<()> {
    let run = store
        .get_run_by_id_prefix(run_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no run found matching '{run_id}'"))?;
    let task = store
        .get_task_by_id(run.task_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task not found"))?;

    let runnable = vigil_registry::deserialize_task(&task.task.runner_type, &task.task.json)?;
    let summary = runnable.summarize_run(&run.metadata);

    if raw {
        if let Some(text) = &summary.result {
            println!("{text}");
        }
        return Ok(());
    }

    crate::format::print_run_header(&task.name, &run, &summary);
    if let Some(text) = &summary.result {
        crate::format::render_markdown(text);
    } else {
        println!("(no output captured)");
    }
    Ok(())
}
