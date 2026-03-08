use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// A task config paired with scheduling metadata. Generic over the task config type.
/// At the DB boundary, `T = RawTask` (untyped JSON). After deserialization, `T` is a concrete
/// task type like `ShellTask` or `ClaudeTask`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask<T> {
    pub id: Uuid,
    pub name: String,
    pub task: T,
    pub trigger: Option<TriggerSpec>,
    pub working_directory: Option<PathBuf>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Untyped task data as stored in the database. The CLI registry is responsible
/// for deserializing `json` into a concrete task type based on `runner_type`.
#[derive(Debug, Clone)]
pub struct RawTask {
    pub runner_type: String,
    pub json: String,
}

/// Canonical scheduling representation, compiled to platform-specific formats by scheduler backends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerSpec {
    Recurring {
        times: Vec<TimeOfDay>,
        days: Option<DayFilter>,
        timezone: Option<String>,
    },
    Interval {
        every: std::time::Duration,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeOfDay {
    pub hour: u8,
    pub minute: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DayFilter {
    Weekdays,
    Weekends,
    Days(Vec<DayOfWeek>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DayOfWeek {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// Tracks a single execution of a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    pub task_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub status: RunStatus,
    pub metadata: HashMap<String, serde_json::Value>,
    pub log_path: PathBuf,
    pub triggered_by: TriggerType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
    TimedOut,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunStatus::Running => write!(f, "running"),
            RunStatus::Succeeded => write!(f, "succeeded"),
            RunStatus::Failed => write!(f, "failed"),
            RunStatus::TimedOut => write!(f, "timed_out"),
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "running" => Ok(RunStatus::Running),
            "succeeded" => Ok(RunStatus::Succeeded),
            "failed" => Ok(RunStatus::Failed),
            "timed_out" => Ok(RunStatus::TimedOut),
            _ => Err(anyhow::anyhow!("unknown run status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerType {
    Manual,
    Schedule,
}

impl std::fmt::Display for TriggerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriggerType::Manual => write!(f, "manual"),
            TriggerType::Schedule => write!(f, "schedule"),
        }
    }
}

impl std::str::FromStr for TriggerType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(TriggerType::Manual),
            "schedule" => Ok(TriggerType::Schedule),
            _ => Err(anyhow::anyhow!("unknown trigger type: {s}")),
        }
    }
}

/// A streaming event emitted by a runner during execution.
#[derive(Debug, Clone)]
pub enum RunEvent {
    /// A line/chunk of output from the runner
    Output {
        text: String,
        /// Optional structured data (runner-specific, e.g. claude event type)
        metadata: Option<serde_json::Value>,
    },
    /// Informational progress hint
    Progress(String),
}

/// Context provided to a runner when executing a task.
#[derive(Debug, Clone)]
pub struct RunContext {
    pub run_id: Uuid,
    pub working_directory: PathBuf,
    pub log_path: PathBuf,
}

/// Output returned by a runner after executing a task.
#[derive(Debug, Clone)]
pub struct RunOutput {
    pub exit_code: i32,
    pub status: RunStatus,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn run_status_display_parse_roundtrip() {
        for status in [
            RunStatus::Running,
            RunStatus::Succeeded,
            RunStatus::Failed,
            RunStatus::TimedOut,
        ] {
            let s = status.to_string();
            let parsed: RunStatus = s.parse().unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn trigger_type_display_parse_roundtrip() {
        for trigger in [TriggerType::Manual, TriggerType::Schedule] {
            let s = trigger.to_string();
            let parsed: TriggerType = s.parse().unwrap();
            assert_eq!(parsed, trigger);
        }
    }

    #[test]
    fn run_status_from_str_invalid_returns_error() {
        assert!(RunStatus::from_str("bogus").is_err());
    }
}
