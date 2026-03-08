//! vigil-registry — composition root: task deserialization, store, and dyn dispatch.

mod store;

pub use store::Store;

use anyhow::{Result, bail};
use tokio::sync::mpsc;
use vigil_core::models::{RunContext, RunEvent, RunOutput};
use vigil_core::runner::{Runner, Task};
use vigil_runner_claude::{ClaudeRunner, ClaudeTask};
use vigil_runner_shell::{ShellRunner, ShellTask};

/// Deserialize a task config from JSON given the runner type discriminant,
/// and pair it with the appropriate runner.
pub fn deserialize_task(runner_type: &str, json: &str) -> Result<Box<dyn Runnable>> {
    match runner_type {
        "shell" => {
            let task: ShellTask = serde_json::from_str(json)?;
            Ok(Box::new(RunnableTask {
                task,
                runner: ShellRunner,
            }))
        }
        "claude" => {
            let task: ClaudeTask = serde_json::from_str(json)?;
            Ok(Box::new(RunnableTask {
                task,
                runner: ClaudeRunner,
            }))
        }
        _ => bail!("unknown runner type: {runner_type}"),
    }
}

/// Dyn-safe wrapper for runnable tasks.
/// This is the dyn boundary used by composition roots (CLI, daemon, etc.) for dispatch,
/// and allows heterogeneous tasks to be handled in the same collection.
#[async_trait::async_trait]
pub trait Runnable: Send + Sync {
    fn runner_type(&self) -> &'static str;
    fn preview(&self, context: &RunContext) -> Result<String>;
    async fn run(&self, context: &RunContext, tx: mpsc::Sender<RunEvent>) -> Result<RunOutput>;
}

/// Composes a Task with its Runner.
struct RunnableTask<T: Task, R: Runner<Task = T>> {
    task: T,
    runner: R,
}

#[async_trait::async_trait]
impl<T: Task, R: Runner<Task = T>> Runnable for RunnableTask<T, R> {
    fn runner_type(&self) -> &'static str {
        self.task.runner_type()
    }

    fn preview(&self, context: &RunContext) -> Result<String> {
        self.runner.preview(&self.task, context)
    }

    async fn run(&self, context: &RunContext, tx: mpsc::Sender<RunEvent>) -> Result<RunOutput> {
        self.runner.run(&self.task, context, tx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_shell_task() {
        let runnable = deserialize_task("shell", r#"{"command":"echo hi"}"#).unwrap();
        assert_eq!(runnable.runner_type(), "shell");
    }

    #[test]
    fn deserialize_claude_task_with_skill() {
        let runnable = deserialize_task("claude", r#"{"skill":"/daily-briefing"}"#).unwrap();
        assert_eq!(runnable.runner_type(), "claude");
    }

    #[test]
    fn deserialize_claude_task_with_prompt() {
        let runnable =
            deserialize_task("claude", r#"{"prompt":"summarize recent git activity"}"#).unwrap();
        assert_eq!(runnable.runner_type(), "claude");
    }

    #[test]
    fn deserialize_unknown_runner_returns_error() {
        assert!(deserialize_task("unknown", "{}").is_err());
    }

    #[test]
    fn deserialize_invalid_json_returns_error() {
        assert!(deserialize_task("shell", "not json").is_err());
    }
}
