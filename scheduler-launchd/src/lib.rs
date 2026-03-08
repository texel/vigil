use anyhow::{Result, bail, Context};
use async_trait::async_trait;
use std::path::PathBuf;
use vigil_core::models::{DayFilter, DayOfWeek, TriggerSpec};
use vigil_core::scheduler::{ScheduleStatus, Scheduler};

fn label(name: &str) -> String {
    format!("com.vigil.task.{name}")
}

fn generate_plist(name: &str, trigger: &TriggerSpec, vigil_bin: &str) -> String {
    let label = label(name);
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let log_dir = format!("{home}/.vigil/logs/{name}");
    let log_path = format!("{log_dir}/launchd.log");

    let trigger_section = match trigger {
        TriggerSpec::Recurring { times, days, .. } => {
            let intervals = build_calendar_intervals(times, days.as_ref());
            format!("<key>StartCalendarInterval</key>\n    <array>\n{intervals}    </array>")
        }
        TriggerSpec::Interval { every } => {
            let secs = every.as_secs();
            format!("<key>StartInterval</key>\n    <integer>{secs}</integer>")
        }
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{vigil_bin}</string>
        <string>run</string>
        <string>{name}</string>
        <string>--quiet</string>
    </array>
    {trigger_section}
    <key>StandardOutPath</key>
    <string>{log_path}</string>
    <key>StandardErrorPath</key>
    <string>{log_path}</string>
    <key>RunAtLoad</key>
    <false/>
</dict>
</plist>
"#
    )
}

fn build_calendar_intervals(
    times: &[vigil_core::models::TimeOfDay],
    days: Option<&DayFilter>,
) -> String {
    let weekdays: Option<Vec<u8>> = days.map(|d| match d {
        DayFilter::Weekdays => vec![1, 2, 3, 4, 5],
        DayFilter::Weekends => vec![0, 6],
        DayFilter::Days(days) => days.iter().map(|d| launchd_weekday(d)).collect(),
    });

    let mut result = String::new();
    for time in times {
        match &weekdays {
            Some(days) => {
                for &day in days {
                    result.push_str(&format!(
                        "        <dict>\n            <key>Weekday</key>\n            <integer>{day}</integer>\n            <key>Hour</key>\n            <integer>{}</integer>\n            <key>Minute</key>\n            <integer>{}</integer>\n        </dict>\n",
                        time.hour, time.minute
                    ));
                }
            }
            None => {
                result.push_str(&format!(
                    "        <dict>\n            <key>Hour</key>\n            <integer>{}</integer>\n            <key>Minute</key>\n            <integer>{}</integer>\n        </dict>\n",
                    time.hour, time.minute
                ));
            }
        }
    }
    result
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
        let plist_content =
            generate_plist(name, trigger, &self.vigil_bin.to_string_lossy());
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
    use vigil_core::models::*;

    #[test]
    fn plist_recurring_weekdays() {
        let trigger = TriggerSpec::Recurring {
            times: vec![TimeOfDay { hour: 9, minute: 0 }],
            days: Some(DayFilter::Weekdays),
            timezone: None,
        };
        let plist = generate_plist("daily-briefing", &trigger, "/usr/local/bin/vigil");
        assert!(plist.contains("<string>com.vigil.task.daily-briefing</string>"));
        assert!(plist.contains("<string>run</string>"));
        assert!(plist.contains("<key>StartCalendarInterval</key>"));
        // Mon-Fri = weekdays 1-5
        assert!(plist.contains("<integer>1</integer>")); // Monday
        assert!(plist.contains("<integer>5</integer>")); // Friday
        assert!(!plist.contains("<key>Weekday</key>\n            <integer>0</integer>")); // No Sunday
        assert!(!plist.contains("<key>Weekday</key>\n            <integer>6</integer>")); // No Saturday
    }

    #[test]
    fn plist_recurring_daily() {
        let trigger = TriggerSpec::Recurring {
            times: vec![TimeOfDay { hour: 8, minute: 30 }],
            days: None,
            timezone: None,
        };
        let plist = generate_plist("morning-task", &trigger, "/usr/local/bin/vigil");
        assert!(plist.contains("<key>StartCalendarInterval</key>"));
        assert!(plist.contains("<integer>8</integer>"));
        assert!(plist.contains("<integer>30</integer>"));
        // No Weekday key for daily
        assert!(!plist.contains("<key>Weekday</key>"));
    }

    #[test]
    fn plist_interval() {
        let trigger = TriggerSpec::Interval {
            every: std::time::Duration::from_secs(3600),
        };
        let plist = generate_plist("health-check", &trigger, "/usr/local/bin/vigil");
        assert!(plist.contains("<key>StartInterval</key>"));
        assert!(plist.contains("<integer>3600</integer>"));
    }

    #[test]
    fn plist_has_quiet_flag() {
        let trigger = TriggerSpec::Interval {
            every: std::time::Duration::from_secs(60),
        };
        let plist = generate_plist("test", &trigger, "/usr/local/bin/vigil");
        assert!(plist.contains("<string>--quiet</string>"));
    }

    #[test]
    fn plist_has_log_paths() {
        let trigger = TriggerSpec::Interval {
            every: std::time::Duration::from_secs(60),
        };
        let plist = generate_plist("my-task", &trigger, "/usr/local/bin/vigil");
        assert!(plist.contains("<key>StandardOutPath</key>"));
        assert!(plist.contains("<key>StandardErrorPath</key>"));
        assert!(plist.contains(".vigil/logs/my-task/launchd.log"));
    }
}
