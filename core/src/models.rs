use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

// Re-export scheduling types from vigil-schedule for backward compatibility.
pub use vigil_schedule::{DayFilter, DayOfWeek, Frequency, TimeOfDay, TriggerSpec};

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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    derive_more::Display,
    derive_more::FromStr,
)]
#[from_str(rename_all = "snake_case")]
#[display(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
    TimedOut,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    derive_more::Display,
    derive_more::FromStr,
)]
#[display(rename_all = "snake_case")]
#[from_str(rename_all = "snake_case")]
pub enum TriggerType {
    Manual,
    Schedule,
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

/// Structured summary of a completed run, for display by any consumer.
#[derive(Debug, Clone, Default)]
pub struct RunSummary {
    /// The main result text (markdown for claude, plain text for shell)
    pub result: Option<String>,
    /// Runner-specific key-value fields to display (e.g. "Tokens" -> "12,340 in / 3,210 out")
    pub fields: Vec<(String, String)>,
}

/// A filtered/transformed streaming event for display.
#[derive(Debug, Clone)]
pub struct EventDisplay {
    /// The text to show (e.g. "> Using tool: Read" or a plain output line)
    pub text: String,
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

    // Scheduling type tests have moved to vigil-schedule.
    // Re-exports are verified by downstream crate compilation.
}
