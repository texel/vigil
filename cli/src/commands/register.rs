use super::RegisterExecutor;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;
use vigil_core::models::{RawTask, ScheduledTask};
use vigil_registry::Store;
use vigil_runner_claude::ClaudeTask;
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
        RegisterExecutor::Claude {
            name,
            prompt_or_skill,
            dir,
            allowed_tools,
            max_turns,
            model,
        } => {
            if store.get_task_by_name(&name).await?.is_some() {
                bail!("task '{name}' already exists");
            }

            let (skill, prompt) = if prompt_or_skill.starts_with('/') {
                (Some(prompt_or_skill), None)
            } else {
                (None, Some(prompt_or_skill))
            };

            let task = ClaudeTask {
                skill,
                prompt,
                allowed_tools: allowed_tools
                    .map(|s| s.split(',').map(|t| t.trim().to_string()).collect()),
                max_turns,
                model,
            };

            let working_directory = match dir {
                Some(d) => Some(PathBuf::from(d)),
                None => Some(std::env::current_dir().context("failed to get current directory")?),
            };
            let now = Utc::now();

            let scheduled = ScheduledTask {
                id: Uuid::new_v4(),
                name: name.clone(),
                task: RawTask {
                    runner_type: "claude".to_string(),
                    json: serde_json::to_string(&task)?,
                },
                trigger: None,
                working_directory,
                enabled: true,
                created_at: now,
                updated_at: now,
            };

            store.insert_task(&scheduled).await?;
            println!("Registered task '{name}' (claude)");
            Ok(())
        }
    }
}
