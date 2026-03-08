use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, derive_more::Display)]
pub enum DayOfWeek {
    #[display("mon")]
    Monday,
    #[display("tue")]
    Tuesday,
    #[display("wed")]
    Wednesday,
    #[display("thu")]
    Thursday,
    #[display("fri")]
    Friday,
    #[display("sat")]
    Saturday,
    #[display("sun")]
    Sunday,
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
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

/// Parses `"HH:MM"` time strings.
///
/// # Examples
///
/// ```
/// use vigil_core::models::TimeOfDay;
/// let t: TimeOfDay = "09:30".parse().unwrap();
/// assert_eq!(t.hour, 9);
/// assert_eq!(t.minute, 30);
/// ```
impl FromStr for TimeOfDay {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            bail!("expected time as HH:MM, got '{s}'");
        }
        let hour: u8 = parts[0]
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid hour: '{}'", parts[0]))?;
        let minute: u8 = parts[1]
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid minute: '{}'", parts[1]))?;
        if hour > 23 {
            bail!("hour must be 0-23, got {hour}");
        }
        if minute > 59 {
            bail!("minute must be 0-59, got {minute}");
        }
        Ok(TimeOfDay { hour, minute })
    }
}

/// Parses day-of-week strings (case-insensitive).
///
/// Accepts three-letter abbreviations (`mon`, `tue`, …) and full names (`monday`, `tuesday`, …).
///
/// # Examples
///
/// ```
/// use vigil_core::models::DayOfWeek;
/// let d: DayOfWeek = "Friday".parse().unwrap();
/// assert_eq!(d, DayOfWeek::Friday);
/// ```
impl FromStr for DayOfWeek {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            "mon" | "monday" => Ok(DayOfWeek::Monday),
            "tue" | "tuesday" => Ok(DayOfWeek::Tuesday),
            "wed" | "wednesday" => Ok(DayOfWeek::Wednesday),
            "thu" | "thursday" => Ok(DayOfWeek::Thursday),
            "fri" | "friday" => Ok(DayOfWeek::Friday),
            "sat" | "saturday" => Ok(DayOfWeek::Saturday),
            "sun" | "sunday" => Ok(DayOfWeek::Sunday),
            _ => bail!("unknown day: '{s}'. Use mon, tue, wed, thu, fri, sat, sun"),
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
            println!("RunStatus as string: {}", s);
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
            DayFilter::Days(vec![
                DayOfWeek::Monday,
                DayOfWeek::Wednesday,
                DayOfWeek::Friday
            ])
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
    fn time_of_day_from_str() {
        let t: TimeOfDay = "14:30".parse().unwrap();
        assert_eq!(t.hour, 14);
        assert_eq!(t.minute, 30);

        assert!("25:00".parse::<TimeOfDay>().is_err());
        assert!("abc".parse::<TimeOfDay>().is_err());
    }

    #[test]
    fn day_of_week_from_str_roundtrip() {
        for day in [
            DayOfWeek::Monday,
            DayOfWeek::Tuesday,
            DayOfWeek::Wednesday,
            DayOfWeek::Thursday,
            DayOfWeek::Friday,
            DayOfWeek::Saturday,
            DayOfWeek::Sunday,
        ] {
            let s = day.to_string();
            let parsed: DayOfWeek = s.parse().unwrap();
            assert_eq!(parsed, day);
        }
    }

    #[test]
    fn day_of_week_from_str_full_names() {
        assert_eq!("Monday".parse::<DayOfWeek>().unwrap(), DayOfWeek::Monday);
        assert_eq!("FRIDAY".parse::<DayOfWeek>().unwrap(), DayOfWeek::Friday);
        assert_eq!("sunday".parse::<DayOfWeek>().unwrap(), DayOfWeek::Sunday);
        assert!("banana".parse::<DayOfWeek>().is_err());
    }

    #[test]
    fn trigger_spec_interval_display() {
        let t = TriggerSpec::Interval {
            every: std::time::Duration::from_secs(7200),
        };
        assert_eq!(t.to_string(), "every 2 hours");
    }
}
