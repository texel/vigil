//! A scheduler implementation for macOS using launchd. This allows Vigil to run
//! tasks in the background without needing a persistent process, and integrates
//! with the system's native scheduling capabilities.

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use vigil_core::scheduler::{ScheduleStatus, Scheduler};
use vigil_schedule::{DayOfWeek, TriggerSpec};

fn label(name: &str) -> String {
    format!("com.vigil.task.{name}")
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LaunchdPlist {
    label: String,
    program_arguments: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment_variables: Option<std::collections::HashMap<String, String>>,
    standard_out_path: String,
    standard_error_path: String,
    run_at_load: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_interval: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_calendar_interval: Option<Vec<CalendarInterval>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CalendarInterval {
    #[serde(skip_serializing_if = "Option::is_none")]
    weekday: Option<u8>,
    hour: u8,
    minute: u8,
}

/// Launchd weekday: 0=Sunday, 1=Monday, ..., 6=Saturday
fn launchd_weekday(day: &DayOfWeek) -> u8 {
    match day {
        DayOfWeek::Sunday => 0,
        DayOfWeek::Monday => 1,
        DayOfWeek::Tuesday => 2,
        DayOfWeek::Wednesday => 3,
        DayOfWeek::Thursday => 4,
        DayOfWeek::Friday => 5,
        DayOfWeek::Saturday => 6,
    }
}

fn generate_plist(name: &str, trigger: &TriggerSpec, vigil_bin: &str) -> Result<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let log_path = format!("{home}/.vigil/logs/{name}/launchd.log");

    let (start_interval, start_calendar_interval) = if let Some(interval) = trigger.interval() {
        (Some(interval.as_secs()), None)
    } else {
        let times = trigger.times_of_day();
        let days = trigger.days_of_week();

        let weekdays: Option<Vec<u8>> = days
            .as_ref()
            .map(|d| d.iter().map(launchd_weekday).collect());

        let mut intervals = Vec::new();
        for time in times {
            match &weekdays {
                Some(days) => {
                    for &day in days {
                        intervals.push(CalendarInterval {
                            weekday: Some(day),
                            hour: time.hour,
                            minute: time.minute,
                        });
                    }
                }
                None => {
                    intervals.push(CalendarInterval {
                        weekday: None,
                        hour: time.hour,
                        minute: time.minute,
                    });
                }
            }
        }
        (None, Some(intervals))
    };

    let environment_variables = std::env::var("PATH").ok().map(|path| {
        let mut m = std::collections::HashMap::new();
        m.insert("PATH".to_string(), path);
        m
    });

    let plist = LaunchdPlist {
        label: label(name),
        program_arguments: vec![
            vigil_bin.to_string(),
            "run".to_string(),
            name.to_string(),
            "--quiet".to_string(),
        ],
        environment_variables,
        standard_out_path: log_path.clone(),
        standard_error_path: log_path,
        run_at_load: false,
        start_interval,
        start_calendar_interval,
    };

    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &plist)?;
    Ok(String::from_utf8(buf)?)
}

pub struct LaunchdScheduler {
    vigil_bin: PathBuf,
    agents_dir: PathBuf,
}

impl LaunchdScheduler {
    pub fn new() -> Result<Self> {
        let vigil_bin = std::env::current_exe().context("failed to get vigil binary path")?;
        let home = std::env::var("HOME").context("HOME not set")?;
        let agents_dir = PathBuf::from(home).join("Library/LaunchAgents");
        Ok(Self {
            vigil_bin,
            agents_dir,
        })
    }

    fn plist_path(&self, name: &str) -> PathBuf {
        self.agents_dir.join(format!("{}.plist", label(name)))
    }

    fn gui_domain(&self) -> String {
        format!("gui/{}", unsafe { libc::getuid() })
    }
}

#[async_trait]
impl Scheduler for LaunchdScheduler {
    async fn register(&self, name: &str, trigger: &TriggerSpec) -> Result<()> {
        let plist_content = generate_plist(name, trigger, &self.vigil_bin.to_string_lossy())?;
        let plist_path = self.plist_path(name);
        let domain = self.gui_domain();

        // Unload existing if present (best-effort)
        let _ = tokio::process::Command::new("launchctl")
            .args(["bootout", &domain, &plist_path.to_string_lossy()])
            .output()
            .await;

        // Ensure log directory exists
        let home = std::env::var("HOME").context("HOME not set")?;
        let log_dir = PathBuf::from(home).join(format!(".vigil/logs/{name}"));
        tokio::fs::create_dir_all(&log_dir).await?;

        // Write plist
        tokio::fs::write(&plist_path, &plist_content).await?;

        // Load
        let output = tokio::process::Command::new("launchctl")
            .args(["bootstrap", &domain, &plist_path.to_string_lossy()])
            .output()
            .await?;
        if !output.status.success() {
            bail!(
                "launchctl bootstrap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        tracing::info!("registered launchd agent: {}", label(name));
        Ok(())
    }

    async fn unregister(&self, name: &str) -> Result<()> {
        let plist_path = self.plist_path(name);
        let domain = self.gui_domain();

        // Bootout (unload) — best-effort
        let _ = tokio::process::Command::new("launchctl")
            .args(["bootout", &domain, &plist_path.to_string_lossy()])
            .output()
            .await;

        // Remove plist file
        if plist_path.exists() {
            tokio::fs::remove_file(&plist_path).await?;
        }

        tracing::info!("unregistered launchd agent: {}", label(name));
        Ok(())
    }

    async fn status(&self, name: &str) -> Result<ScheduleStatus> {
        let plist_path = self.plist_path(name);
        if !plist_path.exists() {
            return Ok(ScheduleStatus::NotScheduled);
        }
        // Check if loaded via launchctl list
        let output = tokio::process::Command::new("launchctl")
            .args(["list", &label(name)])
            .output()
            .await?;
        if output.status.success() {
            Ok(ScheduleStatus::Active)
        } else {
            Ok(ScheduleStatus::NotScheduled)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vigil_schedule::{DayFilter, TimeOfDay};

    fn recurring(times: Vec<TimeOfDay>, days: Option<DayFilter>) -> TriggerSpec {
        let expr = match &days {
            None => format!("daily at {}", times[0]),
            Some(DayFilter::Weekdays) => format!("weekdays at {}", times[0]),
            Some(DayFilter::Weekends) => format!("weekends at {}", times[0]),
            Some(DayFilter::Days(d)) => {
                let day_strs: Vec<String> = d.iter().map(|d| d.to_string()).collect();
                format!("{} at {}", day_strs.join(","), times[0])
            }
        };
        expr.parse().unwrap()
    }

    fn interval(secs: u64) -> TriggerSpec {
        let expr = if secs % 3600 == 0 {
            format!("every {} hours", secs / 3600)
        } else if secs % 60 == 0 {
            format!("every {} minutes", secs / 60)
        } else {
            format!("every {secs} seconds")
        };
        expr.parse().unwrap()
    }

    #[test]
    fn plist_recurring_weekdays() {
        let trigger = recurring(
            vec![TimeOfDay { hour: 9, minute: 0 }],
            Some(DayFilter::Weekdays),
        );
        let plist = generate_plist("daily-briefing", &trigger, "/usr/local/bin/vigil").unwrap();
        assert!(plist.contains("<string>com.vigil.task.daily-briefing</string>"));
        assert!(plist.contains("<string>run</string>"));
        assert!(plist.contains("<key>StartCalendarInterval</key>"));
        // Should have 5 calendar intervals (Mon-Fri)
        assert_eq!(plist.matches("<key>Weekday</key>").count(), 5);
    }

    #[test]
    fn plist_recurring_daily() {
        let trigger = recurring(
            vec![TimeOfDay {
                hour: 8,
                minute: 30,
            }],
            None,
        );
        let plist = generate_plist("morning-task", &trigger, "/usr/local/bin/vigil").unwrap();
        assert!(plist.contains("<key>StartCalendarInterval</key>"));
        assert!(plist.contains("<integer>8</integer>"));
        assert!(plist.contains("<integer>30</integer>"));
        // No Weekday key for daily
        assert!(!plist.contains("<key>Weekday</key>"));
    }

    #[test]
    fn plist_interval() {
        let trigger = interval(3600);
        let plist = generate_plist("health-check", &trigger, "/usr/local/bin/vigil").unwrap();
        assert!(plist.contains("<key>StartInterval</key>"));
        assert!(plist.contains("<integer>3600</integer>"));
    }

    #[test]
    fn plist_has_quiet_flag() {
        let trigger = interval(60);
        let plist = generate_plist("test", &trigger, "/usr/local/bin/vigil").unwrap();
        assert!(plist.contains("<string>--quiet</string>"));
    }

    #[test]
    fn plist_has_log_paths() {
        let trigger = interval(60);
        let plist = generate_plist("my-task", &trigger, "/usr/local/bin/vigil").unwrap();
        assert!(plist.contains("<key>StandardOutPath</key>"));
        assert!(plist.contains("<key>StandardErrorPath</key>"));
        assert!(plist.contains(".vigil/logs/my-task/launchd.log"));
    }

    #[test]
    fn plist_includes_path_env() {
        let trigger = interval(60);
        let plist = generate_plist("test", &trigger, "/usr/local/bin/vigil").unwrap();
        assert!(plist.contains("<key>EnvironmentVariables</key>"));
        assert!(plist.contains("<key>PATH</key>"));
    }

    #[test]
    fn plist_is_valid_xml() {
        let trigger = recurring(
            vec![TimeOfDay { hour: 9, minute: 0 }],
            Some(DayFilter::Days(vec![DayOfWeek::Monday, DayOfWeek::Friday])),
        );
        let xml = generate_plist("xml-test", &trigger, "/usr/local/bin/vigil").unwrap();
        // Should parse back successfully
        let parsed: LaunchdPlist = plist::from_bytes(xml.as_bytes()).unwrap();
        assert_eq!(parsed.label, "com.vigil.task.xml-test");
        assert_eq!(parsed.program_arguments.len(), 4);
        assert!(parsed.environment_variables.is_some());
        assert!(parsed.environment_variables.unwrap().contains_key("PATH"));
        let intervals = parsed.start_calendar_interval.unwrap();
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals[0].weekday, Some(1)); // Monday
        assert_eq!(intervals[1].weekday, Some(5)); // Friday
    }
}
