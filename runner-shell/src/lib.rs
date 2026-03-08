//! vigil-runner-shell — shell command executor for vigil.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use vigil_core::models::{RunContext, RunEvent, RunOutput, RunStatus};
use vigil_core::runner::{Runner, Task};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellTask {
    pub command: String,
}

impl Task for ShellTask {
    fn runner_type(&self) -> &'static str {
        "shell"
    }
}

pub struct ShellRunner;

#[async_trait]
impl Runner for ShellRunner {
    type Task = ShellTask;

    async fn run(
        &self,
        task: &ShellTask,
        context: &RunContext,
        tx: mpsc::Sender<RunEvent>,
    ) -> Result<RunOutput> {
        tracing::info!(
            run_id = %context.run_id,
            command = %task.command,
            "executing shell command"
        );

        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&task.command)
            .current_dir(&context.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn shell command")?;

        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout).lines();
        let mut stdout_lines = Vec::new();

        while let Some(line) = reader.next_line().await? {
            let _ = tx
                .send(RunEvent::Output {
                    text: format!("{line}\n"),
                    metadata: None,
                })
                .await;
            stdout_lines.push(line);
        }

        let output = child.wait_with_output().await?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        let log_content = serde_json::json!({
            "stdout": stdout_lines.join("\n"),
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
