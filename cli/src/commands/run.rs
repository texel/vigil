use anyhow::{bail, Result};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;
use vigil_core::db::Store;
use vigil_core::models::{Run, RunContext, RunStatus, TriggerType};

use crate::{logs_dir, registry};

pub async fn handle(name: &str, store: &Store) -> Result<()> {
    let task_row = store
        .get_task_by_name(name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task '{name}' not found"))?;

    if !task_row.enabled {
        bail!("task '{name}' is disabled");
    }

    let executor = registry::deserialize_config(&task_row.executor_type, &task_row.config_json)?;

    let run_id = Uuid::new_v4();
    let log_dir = logs_dir().join(&task_row.name);
    let log_path = log_dir.join(format!("{}.json", Utc::now().format("%Y-%m-%d_%H-%M-%S")));

    let working_directory = task_row
        .working_directory
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let context = RunContext {
        run_id,
        working_directory,
        log_path: log_path.clone(),
    };

    let mut run = Run {
        id: run_id,
        task_id: task_row.id,
        started_at: Utc::now(),
        completed_at: None,
        exit_code: None,
        status: RunStatus::Running,
        metadata: HashMap::new(),
        log_path,
        triggered_by: TriggerType::Manual,
    };

    store.insert_run(&run).await?;

    println!("Running task '{name}'...");
    let result = executor.execute(&context).await;

    match result {
        Ok(output) => {
            run.exit_code = Some(output.exit_code);
            run.status = output.status;
            run.metadata = output.metadata;
            run.completed_at = Some(Utc::now());
            store.update_run(&run).await?;

            match run.status {
                RunStatus::Succeeded => println!("Task '{name}' succeeded (exit code {})", output.exit_code),
                _ => println!("Task '{name}' finished with status: {} (exit code {})", run.status, output.exit_code),
            }
        }
        Err(e) => {
            run.status = RunStatus::Failed;
            run.completed_at = Some(Utc::now());
            store.update_run(&run).await?;
            println!("Task '{name}' failed: {e:#}");
        }
    }

    Ok(())
}
