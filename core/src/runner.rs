use crate::models::{RunContext, RunOutput};
use anyhow::Result;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;

/// Pure data describing what a runner should execute. Separated from execution behavior.
pub trait Task: Send + Sync + Serialize + DeserializeOwned + Debug + 'static {
    fn runner_type(&self) -> &'static str;
}

/// Stateless execution behavior. Paired with a Task config.
#[async_trait]
pub trait Runner: Send + Sync {
    type Task: Task;

    async fn run(&self, task: &Self::Task, context: &RunContext) -> Result<RunOutput>;
}
