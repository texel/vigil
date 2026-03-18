use anyhow::Result;
use async_trait::async_trait;
use std::fmt;
use vigil_schedule::TriggerSpec;

#[async_trait]
pub trait Scheduler: Send + Sync {
    async fn register(&self, name: &str, trigger: &TriggerSpec) -> Result<()>;
    async fn unregister(&self, name: &str) -> Result<()>;
    async fn status(&self, name: &str) -> Result<ScheduleStatus>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleStatus {
    NotScheduled,
    Active,
}

impl fmt::Display for ScheduleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleStatus::NotScheduled => write!(f, "not scheduled"),
            ScheduleStatus::Active => write!(f, "active"),
        }
    }
}
