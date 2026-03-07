use crate::models::{RunContext, RunOutput, TriggerSpec};
use anyhow::Result;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use uuid::Uuid;

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

/// Dyn-safe boundary for heterogeneous task collections.
/// The scheduler and storage layers work with `Box<dyn Schedulable>`.
#[async_trait]
pub trait Schedulable: Send + Sync {
    fn task_id(&self) -> Uuid;
    fn task_name(&self) -> &str;
    fn runner_type(&self) -> &'static str;
    fn trigger(&self) -> Option<&TriggerSpec>;
    async fn run(&self, context: &RunContext) -> Result<RunOutput>;
}
