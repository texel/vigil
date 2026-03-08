use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
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

impl fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

impl fmt::Display for DayOfWeek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DayOfWeek::Monday => write!(f, "mon"),
            DayOfWeek::Tuesday => write!(f, "tue"),
            DayOfWeek::Wednesday => write!(f, "wed"),
            DayOfWeek::Thursday => write!(f, "thu"),
            DayOfWeek::Friday => write!(f, "fri"),
            DayOfWeek::Saturday => write!(f, "sat"),
            DayOfWeek::Sunday => write!(f, "sun"),
        }
    }
}

impl fmt::Display for DayFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DayFilter::Weekdays => write!(f, "weekdays"),
            DayFilter::Weekends => write!(f, "weekends"),
            DayFilter::Days(days) => {
                let names: Vec<String> = days.iter().map(|d| d.to_string()).collect();
                write!(f, "{}", names.join(","))
            }
        }
    }
}

impl fmt::Display for TriggerSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriggerSpec::Recurring {
                times,
                days,
                timezone,
            } => {
                if let Some(days) = days {
                    write!(f, "{days}")?;
                } else {
                    write!(f, "daily")?;
                }
                if !times.is_empty() {
                    let time_strs: Vec<String> = times.iter().map(|t| t.to_string()).collect();
                    write!(f, " at {}", time_strs.join(","))?;
                }
                if let Some(tz) = timezone {
                    write!(f, " ({tz})")?;
                }
                Ok(())
            }
            TriggerSpec::Interval { every } => {
                let secs = every.as_secs();
                if secs % 3600 == 0 {
                    let hours = secs / 3600;
                    write!(f, "every {hours} hours")
                } else if secs % 60 == 0 {
                    let mins = secs / 60;
                    write!(f, "every {mins} minutes")
                } else {
                    write!(f, "every {secs} seconds")
                }
            }
        }
    }
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

    #[test]
    fn time_of_day_display() {
        let t = TimeOfDay { hour: 9, minute: 5 };
        assert_eq!(t.to_string(), "09:05");
    }

    #[test]
    fn day_of_week_display() {
        assert_eq!(DayOfWeek::Monday.to_string(), "mon");
        assert_eq!(DayOfWeek::Sunday.to_string(), "sun");
    }

    #[test]
    fn day_filter_display() {
        assert_eq!(DayFilter::Weekdays.to_string(), "weekdays");
        assert_eq!(DayFilter::Weekends.to_string(), "weekends");
        assert_eq!(
            DayFilter::Days(vec![DayOfWeek::Monday, DayOfWeek::Wednesday, DayOfWeek::Friday])
                .to_string(),
            "mon,wed,fri"
        );
    }

    #[test]
    fn trigger_spec_recurring_display() {
        let t = TriggerSpec::Recurring {
            times: vec![TimeOfDay { hour: 9, minute: 0 }],
            days: Some(DayFilter::Weekdays),
            timezone: None,
        };
        assert_eq!(t.to_string(), "weekdays at 09:00");
    }

    #[test]
    fn trigger_spec_interval_display() {
        let t = TriggerSpec::Interval {
            every: std::time::Duration::from_secs(7200),
        };
        assert_eq!(t.to_string(), "every 2 hours");
    }
}
