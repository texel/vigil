use super::RegisterExecutor;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;
use vigil_core::db::{Store, TaskRow};
use vigil_executor_shell::ShellConfig;

pub async fn handle(executor: RegisterExecutor, store: &Store) -> Result<()> {
    match executor {
        RegisterExecutor::Shell { name, command, dir } => {
            if store.get_task_by_name(&name).await?.is_some() {
                bail!("task '{name}' already exists");
            }

            let config = ShellConfig { command };
            let working_directory = match dir {
                Some(d) => Some(PathBuf::from(d)),
                None => Some(std::env::current_dir().context("failed to get current directory")?),
            };
            let now = Utc::now();

            let row = TaskRow {
                id: Uuid::new_v4(),
                name: name.clone(),
                executor_type: "shell".to_string(),
                config_json: serde_json::to_string(&config)?,
                trigger_json: None,
                working_directory,
                enabled: true,
                created_at: now,
                updated_at: now,
            };

            store.insert_task(&row).await?;
            println!("Registered task '{name}' (shell)");
            Ok(())
        }
    }
}
