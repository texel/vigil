use super::RegisterExecutor;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;
use vigil_core::db::Store;
use vigil_core::models::{RawTask, ScheduledTask};
use vigil_runner_shell::ShellTask;

pub async fn handle(executor: RegisterExecutor, store: &Store) -> Result<()> {
    match executor {
        RegisterExecutor::Shell { name, command, dir } => {
            if store.get_task_by_name(&name).await?.is_some() {
                bail!("task '{name}' already exists");
            }

            let task = ShellTask { command };
            let working_directory = match dir {
                Some(d) => Some(PathBuf::from(d)),
                None => Some(std::env::current_dir().context("failed to get current directory")?),
            };
            let now = Utc::now();

            let scheduled = ScheduledTask {
                id: Uuid::new_v4(),
                name: name.clone(),
                task: RawTask {
                    runner_type: "shell".to_string(),
                    json: serde_json::to_string(&task)?,
                },
                trigger: None,
                working_directory,
                enabled: true,
                created_at: now,
                updated_at: now,
            };

            store.insert_task(&scheduled).await?;
            println!("Registered task '{name}' (shell)");
            Ok(())
        }
    }
}
