//! vigil-runner-claude — Claude Code executor for vigil.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use vigil_core::models::{RunContext, RunOutput, RunStatus};
use vigil_core::runner::{Runner, Task};

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
}

impl Task for ClaudeTask {
    fn runner_type(&self) -> &'static str {
        "claude"
    }
}

pub struct ClaudeRunner;

#[async_trait]
impl Runner for ClaudeRunner {
    type Task = ClaudeTask;

    async fn run(&self, task: &ClaudeTask, context: &RunContext) -> Result<RunOutput> {
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
        cmd.arg("-p")
            .arg(prompt_text)
            .arg("--output-format")
            .arg("json")
            .arg("--session-id")
            .arg(context.run_id.to_string());

        if let Some(ref tools) = task.allowed_tools {
            for tool in tools {
                cmd.arg("--allowedTools").arg(tool);
            }
        }

        if let Some(max_turns) = task.max_turns {
            cmd.arg("--max-turns").arg(max_turns.to_string());
        }

        if let Some(ref model) = task.model {
            cmd.arg("--model").arg(model);
        }

        cmd.current_dir(&context.working_directory);

        let output = cmd.output().await.context("failed to spawn claude command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Write full output to log file
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

        // Extract metadata from JSON output
        let mut metadata = HashMap::new();
        metadata.insert(
            "session_id".to_string(),
            serde_json::Value::String(context.run_id.to_string()),
        );

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let Some(session_id) = json.get("session_id").and_then(|v| v.as_str()) {
                metadata.insert(
                    "session_id".to_string(),
                    serde_json::Value::String(session_id.to_string()),
                );
            }
            if let Some(usage) = json.get("usage") {
                metadata.insert("usage".to_string(), usage.clone());
            }
            if let Some(result) = json.get("result") {
                metadata.insert("result".to_string(), result.clone());
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
