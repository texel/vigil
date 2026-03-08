use crate::models::{RunContext, RunEvent, RunOutput};
use anyhow::Result;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use tokio::sync::mpsc;

/// Pure data describing what a runner should execute. Separated from execution behavior.
pub trait Task: Send + Sync + Serialize + DeserializeOwned + Debug + 'static {
    fn runner_type(&self) -> &'static str;
}

/// Stateless execution behavior. Paired with a Task config.
#[async_trait]
pub trait Runner: Send + Sync {
    type Task: Task;

    /// Returns a human-readable description of what the runner would do.
    fn preview(&self, task: &Self::Task, context: &RunContext) -> Result<String>;

    async fn run(
        &self,
        task: &Self::Task,
        context: &RunContext,
        tx: mpsc::Sender<RunEvent>,
    ) -> Result<RunOutput>;
}
