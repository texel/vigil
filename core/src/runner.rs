use crate::models::{RunContext, RunEvent, RunOutput};
use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use tokio::sync::mpsc;

/// Data describing what a runner should execute.
pub trait Task: Send + Sync + Serialize + DeserializeOwned + Debug + 'static {
    fn runner_type(&self) -> &'static str;
}

/// A Runner can run tasks.
#[async_trait]
pub trait Runner: Send + Sync {
    type Task: Task;

    /// Returns a human-readable description of what the runner would do
    /// when calling `run`. Useful for logging, debugging, and dry-run functionality.
    fn preview(&self, task: &Self::Task, context: &RunContext) -> Result<String>;

    async fn run(
        &self,
        task: &Self::Task,
        context: &RunContext,
        tx: mpsc::Sender<RunEvent>,
    ) -> Result<RunOutput>;
}
