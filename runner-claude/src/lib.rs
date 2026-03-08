//! Claude Code runner for vigil.
//!
//! This crate implements a Runner that can execute tasks using the `claude` CLI
//! tool. It supports both skill-based and prompt-based tasks, captures
//! streaming JSON output, and extracts metadata for integration with the vigil
//! system.

use anyhow::{Context, Result};
use async_trait::async_trait;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use vigil_core::models::{RunContext, RunEvent, RunOutput, RunStatus};
use vigil_core::runner::{Runner, Task};

const RUNNER_TYPE: &str = "claude";

#[derive(Debug, Clone, Serialize, Deserialize, Default, Display)]
#[display(rename_all = "camelCase")]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    AcceptEdits,
    BypassPermissions,
    #[serde(rename = "default")]
    Default,
    #[default]
    DontAsk,
    Plan,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<PermissionMode>,
}

impl Task for ClaudeTask {
    fn runner_type(&self) -> &'static str {
        RUNNER_TYPE
    }
}

pub struct ClaudeRunner;

impl ClaudeRunner {
    /// Build the argument list for a claude invocation.
    fn build_args(task: &ClaudeTask, context: &RunContext) -> Result<Vec<String>> {
        let prompt_text = task
            .skill
            .as_deref()
            .or(task.prompt.as_deref())
            .context("claude task must have either a skill or prompt")?;

        let mut args = vec![
            "-p".to_string(),
            prompt_text.to_string(),
            "--verbose".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--permission-mode".to_string(),
            task.permission_mode
                .as_ref()
                .unwrap_or(&PermissionMode::default())
                .to_string(),
            "--session-id".to_string(),
            context.run_id.to_string(),
        ];

        if let Some(ref tools) = task.allowed_tools {
            for tool in tools {
                args.push("--allowedTools".to_string());
                args.push(tool.clone());
            }
        }

        if let Some(max_turns) = task.max_turns {
            args.push("--max-turns".to_string());
            args.push(max_turns.to_string());
        }

        if let Some(ref model) = task.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        Ok(args)
    }
}

#[async_trait]
impl Runner for ClaudeRunner {
    type Task = ClaudeTask;

    fn preview(&self, task: &ClaudeTask, context: &RunContext) -> Result<String> {
        let args = Self::build_args(task, context)?;
        let escaped: Vec<String> = args
            .iter()
            .map(|a| shell_escape::escape(a.into()).into_owned())
            .collect();
        Ok(format!(
            "cd {} && claude {}",
            context.working_directory.display(),
            escaped.join(" ")
        ))
    }

    async fn run(
        &self,
        task: &ClaudeTask,
        context: &RunContext,
        tx: mpsc::Sender<RunEvent>,
    ) -> Result<RunOutput> {
        let args = Self::build_args(task, context)?;

        let prompt_text = task
            .skill
            .as_deref()
            .or(task.prompt.as_deref())
            .context("claude task must have either a skill or prompt")?;

        tracing::info!(
            run_id = %context.run_id,
            prompt = %prompt_text,
            "executing claude command"
        );

        let mut cmd = tokio::process::Command::new("claude");
        for arg in &args {
            cmd.arg(arg);
        }

        cmd.current_dir(&context.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("failed to spawn claude command")?;

        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout).lines();
        let mut raw_lines = Vec::new();
        let mut result_event: Option<serde_json::Value> = None;

        while let Some(line) = reader.next_line().await? {
            raw_lines.push(line.clone());

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                // Capture the result event for metadata extraction
                if json.get("type").and_then(|v| v.as_str()) == Some("result") {
                    result_event = Some(json.clone());
                }

                let _ = tx
                    .send(RunEvent::Output {
                        text: format!("{line}\n"),
                        metadata: Some(json),
                    })
                    .await;
            } else {
                let _ = tx
                    .send(RunEvent::Output {
                        text: format!("{line}\n"),
                        metadata: None,
                    })
                    .await;
            }
        }

        let output = child.wait_with_output().await?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Write full output to log file
        let log_content = serde_json::json!({
            "stdout": raw_lines.join("\n"),
            "stderr": stderr,
            "exit_code": output.status.code(),
        });
        if let Some(parent) = context.log_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(
            &context.log_path,
            serde_json::to_string_pretty(&log_content)?,
        )
        .await
        .context("failed to write log file")?;

        // Extract metadata from result event
        let mut metadata = HashMap::new();
        metadata.insert(
            "session_id".to_string(),
            serde_json::Value::String(context.run_id.to_string()),
        );

        if let Some(ref result) = result_event {
            if let Some(session_id) = result.get("session_id").and_then(|v| v.as_str()) {
                metadata.insert(
                    "session_id".to_string(),
                    serde_json::Value::String(session_id.to_string()),
                );
            }
            if let Some(usage) = result.get("usage") {
                metadata.insert("usage".to_string(), usage.clone());
            }
            if let Some(r) = result.get("result") {
                metadata.insert("result".to_string(), r.clone());
            }
        }

        let exit_code = output.status.code().unwrap_or(-1);
        let status = if output.status.success() {
            RunStatus::Succeeded
        } else {
            RunStatus::Failed
        };

        Ok(RunOutput {
            exit_code,
            status,
            metadata,
        })
    }
}
