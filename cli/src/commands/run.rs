use anyhow::{bail, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;
use vigil_core::models::{Run, RunContext, RunEvent, RunStatus, TriggerType};
use vigil_registry::Store;

use crate::logs_dir;

pub async fn handle(name: &str, quiet: bool, dry_run: bool, store: &Store) -> Result<()> {
    let scheduled = store
        .get_task_by_name(name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task '{name}' not found"))?;

    if !scheduled.enabled {
        bail!("task '{name}' is disabled");
    }

    let runnable: Arc<Box<dyn vigil_registry::Runnable>> = Arc::new(
        vigil_registry::deserialize_task(&scheduled.task.runner_type, &scheduled.task.json)?,
    );

    let run_id = Uuid::new_v4();
    let log_dir = logs_dir().join(&scheduled.name);
    let log_path = log_dir.join(format!("{}.json", Utc::now().format("%Y-%m-%d_%H-%M-%S")));

    let working_directory = scheduled
        .working_directory
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let context = RunContext {
        run_id,
        working_directory,
        log_path: log_path.clone(),
    };

    if dry_run {
        let preview = runnable.preview(&context)?;
        println!("{preview}");
        return Ok(());
    }

    let mut run = Run {
        id: run_id,
        task_id: scheduled.id,
        started_at: Utc::now(),
        completed_at: None,
        exit_code: None,
        status: RunStatus::Running,
        metadata: HashMap::new(),
        log_path,
        triggered_by: TriggerType::Manual,
    };

    store.insert_run(&run).await?;

    let (tx, mut rx) = mpsc::channel::<RunEvent>(64);

    if !quiet {
        let runnable_clone = Arc::clone(&runnable);
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Some(display) = runnable_clone.format_event(&event) {
                    eprint!("{}", display.text);
                }
            }
        });
    } else {
        drop(rx);
    }

    eprintln!("Running task '{name}'...");
    let result = runnable.run(&context, tx).await;

    match result {
        Ok(output) => {
            run.exit_code = Some(output.exit_code);
            run.status = output.status;
            run.metadata = output.metadata;
            run.completed_at = Some(Utc::now());
            store.update_run(&run).await?;

            match run.status {
                RunStatus::Succeeded => {
                    eprintln!("Task '{name}' succeeded (exit code {})", output.exit_code);
                    let summary = runnable.summarize_run(&run.metadata);
                    if let Some(text) = &summary.result {
                        crate::format::render_markdown(text);
                    }
                }
                _ => eprintln!(
                    "Task '{name}' finished with status: {} (exit code {})",
                    run.status, output.exit_code
                ),
            }
        }
        Err(e) => {
            run.status = RunStatus::Failed;
            run.completed_at = Some(Utc::now());
            store.update_run(&run).await?;
            eprintln!("Task '{name}' failed: {e:#}");
        }
    }

    Ok(())
}
