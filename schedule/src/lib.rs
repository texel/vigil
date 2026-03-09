pub mod models;
mod parser;
mod rrule_convert;

pub use models::{DayFilter, DayOfWeek, Frequency, TimeOfDay};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rrule::RRuleSet;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

/// Canonical scheduling representation. Internally stored as an RRULE string,
/// with pre-parsed typed fields for efficient accessor use by scheduler backends.
///
/// Scheduler backends should use the typed accessors (`times_of_day()`, `days_of_week()`,
/// `interval()`) rather than parsing the RRULE string directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TriggerSpec {
    /// Time-based recurring schedule, internally stored as RRULE.
    RRule {
        rrule: String,
        /// RFC 5545 DTSTART string (e.g. `"20200101T090000Z"`). Required for
        /// RRULE evaluation. Defaults to empty for backward compat with old DB rows;
        /// `next_occurrence()` synthesizes one when missing.
        #[serde(default)]
        dtstart: String,
        times: Vec<TimeOfDay>,
        days: Option<DayFilter>,
        /// For interval-based schedules, the interval in seconds.
        interval_secs: Option<u64>,
    },
}

impl TriggerSpec {
    /// The times of day this trigger fires. Empty for interval-based schedules.
    pub fn times_of_day(&self) -> &[TimeOfDay] {
        let TriggerSpec::RRule { times, .. } = self;
        times
    }

    /// The days of week this trigger fires, if constrained.
    /// `None` means every day (daily). Returns expanded days for Weekdays/Weekends.
    pub fn days_of_week(&self) -> Option<Vec<DayOfWeek>> {
        let TriggerSpec::RRule { days, .. } = self;
        days.as_ref().map(|d| match d {
            DayFilter::Weekdays => vec![
                DayOfWeek::Monday,
                DayOfWeek::Tuesday,
                DayOfWeek::Wednesday,
                DayOfWeek::Thursday,
                DayOfWeek::Friday,
            ],
            DayFilter::Weekends => vec![DayOfWeek::Saturday, DayOfWeek::Sunday],
            DayFilter::Days(days) => days.clone(),
        })
    }

    /// The raw `DayFilter` if this is a recurring schedule with day constraints.
    pub fn day_filter(&self) -> Option<&DayFilter> {
        let TriggerSpec::RRule { days, .. } = self;
        days.as_ref()
    }

    /// For interval-based schedules, returns the interval duration.
    pub fn interval(&self) -> Option<Duration> {
        let TriggerSpec::RRule { interval_secs, .. } = self;
        interval_secs.map(Duration::from_secs)
    }

    /// The frequency of this schedule.
    pub fn frequency(&self) -> Frequency {
        let TriggerSpec::RRule {
            interval_secs,
            days,
            ..
        } = self;
        if interval_secs.is_some() {
            Frequency::Hourly // interval-based
        } else if days.is_some() {
            Frequency::Weekly
        } else {
            Frequency::Daily
        }
    }

    /// The raw RRULE string.
    pub fn to_rrule_string(&self) -> &str {
        let TriggerSpec::RRule { rrule, .. } = self;
        rrule
    }

    /// The DTSTART string for this schedule.
    pub fn dtstart(&self) -> &str {
        let TriggerSpec::RRule { dtstart, .. } = self;
        dtstart
    }

    /// Compute the next occurrence of this schedule after `after`.
    ///
    /// Parses the stored RRULE + DTSTART via the `rrule` crate and returns the
    /// first occurrence strictly after the given time.
    pub fn next_occurrence(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let TriggerSpec::RRule {
            rrule,
            dtstart,
            times,
            ..
        } = self;

        let dtstart_str = if dtstart.is_empty() {
            rrule_convert::default_dtstart(times)
        } else {
            dtstart.clone()
        };

        let input = format!("DTSTART:{dtstart_str}\n{rrule}");
        let rrule_set: RRuleSet = input.parse().ok()?;

        rrule_set
            .after(after.with_timezone(&rrule::Tz::UTC))
            .all(1)
            .dates
            .into_iter()
            .next()
            .map(|dt| dt.with_timezone(&Utc))
    }
}

/// Parse NLP trigger expressions like `"daily at 9am"` or `"every 2 hours"`.
impl FromStr for TriggerSpec {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        parser::parse_trigger(s)
    }
}

impl fmt::Display for TriggerSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let TriggerSpec::RRule {
            times,
            days,
            interval_secs,
            ..
        } = self;

        if let Some(secs) = interval_secs {
            return if secs % 3600 == 0 {
                let n = secs / 3600;
                let unit = if n == 1 { "hour" } else { "hours" };
                write!(f, "every {n} {unit}")
            } else if secs % 60 == 0 {
                let n = secs / 60;
                let unit = if n == 1 { "minute" } else { "minutes" };
                write!(f, "every {n} {unit}")
            } else {
                let unit = if *secs == 1 { "second" } else { "seconds" };
                write!(f, "every {secs} {unit}")
            };
        }

        match days {
            Some(days) => write!(f, "{days}")?,
            None => write!(f, "daily")?,
        }
        if !times.is_empty() {
            let time_strs: Vec<String> =
                times.iter().map(|t| t.to_string()).collect();
            write!(f, " at {}", time_strs.join(","))?;
        }
        Ok(())
    }
}

/// Deserialize legacy `TriggerSpec` formats from the database.
///
/// The old format used `{"Recurring": {"times": [...], "days": ..., "timezone": ...}}`
/// and `{"Interval": {"every": {"secs": N, "nanos": 0}}}`.
/// This function tries the current format first, then falls back to the legacy format.
pub fn deserialize_trigger_compat(json: &str) -> Result<TriggerSpec> {
    // Try current format first
    if let Ok(trigger) = serde_json::from_str::<TriggerSpec>(json) {
        return Ok(trigger);
    }

    // Try legacy format
    let legacy: serde_json::Value = serde_json::from_str(json)?;

    if let Some(recurring) = legacy.get("Recurring") {
        let times: Vec<TimeOfDay> = serde_json::from_value(
            recurring.get("times").cloned().unwrap_or_default(),
        )?;
        let days: Option<DayFilter> = recurring
            .get("days")
            .filter(|v| !v.is_null())
            .cloned()
            .map(serde_json::from_value)
            .transpose()?;
        let rrule = rrule_convert::build_recurring_rrule(&times, days.as_ref());
        let dtstart = rrule_convert::default_dtstart(&times);
        return Ok(TriggerSpec::RRule {
            rrule,
            dtstart,
            times,
            days,
            interval_secs: None,
        });
    }

    if let Some(interval) = legacy.get("Interval") {
        let every = interval.get("every").ok_or_else(|| {
            anyhow::anyhow!("legacy Interval missing 'every' field")
        })?;
        let secs = every
            .get("secs")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("legacy Interval missing 'secs'"))?;
        let rrule = rrule_convert::build_interval_rrule(secs);
        let dtstart = rrule_convert::default_dtstart(&[]);
        return Ok(TriggerSpec::RRule {
            rrule,
            dtstart,
            times: vec![],
            days: None,
            interval_secs: Some(secs),
        });
    }

    anyhow::bail!("unrecognized trigger JSON format: {json}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn from_str_daily() {
        let t: TriggerSpec = "daily at 09:00".parse().unwrap();
        assert_eq!(t.times_of_day()[0].hour, 9);
        assert!(t.days_of_week().is_none());
        assert_eq!(t.frequency(), Frequency::Daily);
    }

    #[test]
    fn from_str_12h() {
        let t: TriggerSpec = "weekdays at 9am".parse().unwrap();
        assert_eq!(t.times_of_day()[0].hour, 9);
        assert_eq!(t.days_of_week().unwrap().len(), 5);
        assert_eq!(t.frequency(), Frequency::Weekly);
    }

    #[test]
    fn from_str_interval() {
        let t: TriggerSpec = "every 2 hours".parse().unwrap();
        assert_eq!(t.interval(), Some(Duration::from_secs(7200)));
        assert!(t.times_of_day().is_empty());
    }

    #[test]
    fn display_recurring() {
        let t: TriggerSpec = "weekdays at 09:00".parse().unwrap();
        assert_eq!(t.to_string(), "weekdays at 09:00");
    }

    #[test]
    fn display_daily() {
        let t: TriggerSpec = "daily at 14:30".parse().unwrap();
        assert_eq!(t.to_string(), "daily at 14:30");
    }

    #[test]
    fn display_interval() {
        let t: TriggerSpec = "every 2 hours".parse().unwrap();
        assert_eq!(t.to_string(), "every 2 hours");
    }

    #[test]
    fn rrule_string() {
        let t: TriggerSpec = "weekdays at 9am".parse().unwrap();
        assert_eq!(t.to_rrule_string(), "RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR");
    }

    #[test]
    fn legacy_recurring_compat() {
        let json = r#"{"Recurring":{"times":[{"hour":9,"minute":0}],"days":"Weekdays","timezone":null}}"#;
        let t = deserialize_trigger_compat(json).unwrap();
        assert_eq!(t.times_of_day()[0].hour, 9);
        assert!(t.days_of_week().is_some());
    }

    #[test]
    fn legacy_interval_compat() {
        let json = r#"{"Interval":{"every":{"secs":7200,"nanos":0}}}"#;
        let t = deserialize_trigger_compat(json).unwrap();
        assert_eq!(t.interval(), Some(Duration::from_secs(7200)));
    }

    #[test]
    fn new_format_roundtrip() {
        let t: TriggerSpec = "weekdays at 9am".parse().unwrap();
        let json = serde_json::to_string(&t).unwrap();
        let t2: TriggerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(t2.times_of_day()[0].hour, 9);
        assert_eq!(t2.to_rrule_string(), t.to_rrule_string());
    }

    #[test]
    fn next_occurrence_daily() {
        let t: TriggerSpec = "daily at 09:00".parse().unwrap();
        // Ask for next after 2026-03-08 08:00 UTC → should be 2026-03-08 09:00
        let after = Utc.with_ymd_and_hms(2026, 3, 8, 8, 0, 0).unwrap();
        let next = t.next_occurrence(after).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 3, 8, 9, 0, 0).unwrap());
    }

    #[test]
    fn next_occurrence_daily_after_time() {
        let t: TriggerSpec = "daily at 09:00".parse().unwrap();
        // Ask for next after 2026-03-08 10:00 UTC → should be 2026-03-09 09:00
        let after = Utc.with_ymd_and_hms(2026, 3, 8, 10, 0, 0).unwrap();
        let next = t.next_occurrence(after).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 3, 9, 9, 0, 0).unwrap());
    }

    #[test]
    fn next_occurrence_weekly_with_days() {
        let t: TriggerSpec = "mon,wed,fri at 14:00".parse().unwrap();
        // 2026-03-08 is a Sunday → next should be Monday 2026-03-09 14:00
        let after = Utc.with_ymd_and_hms(2026, 3, 8, 12, 0, 0).unwrap();
        let next = t.next_occurrence(after).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 3, 9, 14, 0, 0).unwrap());
    }

    #[test]
    fn next_occurrence_interval() {
        let t: TriggerSpec = "every 2 hours".parse().unwrap();
        let after = Utc.with_ymd_and_hms(2026, 3, 8, 10, 30, 0).unwrap();
        let next = t.next_occurrence(after).unwrap();
        // Interval-based: every 2 hours from DTSTART midnight → 0, 2, 4, 6, 8, 10, 12...
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 3, 8, 12, 0, 0).unwrap());
    }

    #[test]
    fn next_occurrence_midnight_edge() {
        let t: TriggerSpec = "daily at midnight".parse().unwrap();
        // Just before midnight
        let after = Utc.with_ymd_and_hms(2026, 3, 8, 23, 59, 0).unwrap();
        let next = t.next_occurrence(after).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 3, 9, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_occurrence_missing_dtstart_compat() {
        // Simulate old DB row without dtstart field
        let json = r#"{"type":"RRule","rrule":"RRULE:FREQ=DAILY","times":[{"hour":9,"minute":0}],"days":null,"interval_secs":null}"#;
        let t: TriggerSpec = serde_json::from_str(json).unwrap();
        assert!(t.dtstart().is_empty());
        // Should still compute next_occurrence by synthesizing a dtstart
        let after = Utc.with_ymd_and_hms(2026, 3, 8, 8, 0, 0).unwrap();
        let next = t.next_occurrence(after);
        assert!(next.is_some());
    }
}
