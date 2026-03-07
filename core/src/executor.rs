use crate::models::{RunContext, RunOutput, TriggerSpec};
use anyhow::Result;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use uuid::Uuid;

/// Pure data describing how an executor should run. Separated from execution behavior.
pub trait ExecutorConfig: Send + Sync + Serialize + DeserializeOwned + Debug + 'static {
    fn executor_type(&self) -> &'static str;
}

/// Stateless execution behavior. Paired with an ExecutorConfig.
#[async_trait]
pub trait Executor: Send + Sync {
    type Config: ExecutorConfig;

    async fn execute(&self, config: &Self::Config, context: &RunContext) -> Result<RunOutput>;
}

/// Dyn-safe boundary for heterogeneous task collections.
/// The scheduler and storage layers work with `Box<dyn Schedulable>`.
#[async_trait]
pub trait Schedulable: Send + Sync {
    fn task_id(&self) -> Uuid;
    fn task_name(&self) -> &str;
    fn executor_type(&self) -> &'static str;
    fn trigger(&self) -> Option<&TriggerSpec>;
    async fn execute(&self, context: &RunContext) -> Result<RunOutput>;
}
