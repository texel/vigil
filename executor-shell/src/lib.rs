use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vigil_core::executor::{Executor, ExecutorConfig};
use vigil_core::models::{RunContext, RunOutput, RunStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    pub command: String,
}

impl ExecutorConfig for ShellConfig {
    fn executor_type(&self) -> &'static str {
        "shell"
    }
}

pub struct ShellExecutor;

#[async_trait]
impl Executor for ShellExecutor {
    type Config = ShellConfig;

    async fn execute(&self, config: &ShellConfig, context: &RunContext) -> Result<RunOutput> {
        tracing::info!(
            run_id = %context.run_id,
            command = %config.command,
            "executing shell command"
        );

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&config.command)
            .current_dir(&context.working_directory)
            .output()
            .await
            .context("failed to spawn shell command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Write output to log file
        let log_content = serde_json::json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": output.status.code(),
        });
        if let Some(parent) = context.log_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&context.log_path, serde_json::to_string_pretty(&log_content)?)
            .await
            .context("failed to write log file")?;

        let exit_code = output.status.code().unwrap_or(-1);
        let status = if output.status.success() {
            RunStatus::Succeeded
        } else {
            RunStatus::Failed
        };

        Ok(RunOutput {
            exit_code,
            status,
            metadata: Default::default(),
        })
    }
}
