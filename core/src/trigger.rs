use crate::models::{DayFilter, DayOfWeek, TimeOfDay, TriggerSpec};
use anyhow::{bail, Result};
use std::time::Duration;

pub fn parse_trigger(input: &str) -> Result<TriggerSpec> {
    let input = input.trim().to_lowercase();

    // "every N unit" pattern
    if input.starts_with("every ") {
        return parse_interval(&input[6..]);
    }

    // "<days> at HH:MM" pattern
    if let Some(at_pos) = input.find(" at ") {
        let days_part = &input[..at_pos];
        let time_part = &input[at_pos + 4..];
        let time = parse_time(time_part)?;
        let days = parse_days(days_part)?;
        return Ok(TriggerSpec::Recurring {
            times: vec![time],
            days,
            timezone: None,
        });
    }

    bail!("unrecognized trigger format: '{input}'. Try 'daily at 09:00', 'weekdays at 09:00', or 'every 2 hours'")
}

fn parse_interval(s: &str) -> Result<TriggerSpec> {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    if parts.len() != 2 {
        bail!("expected 'every N unit', got 'every {s}'");
    }
    let n: u64 = parts[0].parse().map_err(|_| anyhow::anyhow!("invalid number: '{}'", parts[0]))?;
    if n == 0 {
        bail!("interval must be greater than 0");
    }
    let secs = match parts[1] {
        "hour" | "hours" => n * 3600,
        "minute" | "minutes" | "min" | "mins" => n * 60,
        "second" | "seconds" | "sec" | "secs" => n,
        unit => bail!("unknown time unit: '{unit}'. Use hours, minutes, or seconds"),
    };
    Ok(TriggerSpec::Interval {
        every: Duration::from_secs(secs),
    })
}

fn parse_time(s: &str) -> Result<TimeOfDay> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.len() != 2 {
        bail!("expected time as HH:MM, got '{s}'");
    }
    let hour: u8 = parts[0].parse().map_err(|_| anyhow::anyhow!("invalid hour: '{}'", parts[0]))?;
    let minute: u8 = parts[1].parse().map_err(|_| anyhow::anyhow!("invalid minute: '{}'", parts[1]))?;
    if hour > 23 {
        bail!("hour must be 0-23, got {hour}");
    }
    if minute > 59 {
        bail!("minute must be 0-59, got {minute}");
    }
    Ok(TimeOfDay { hour, minute })
}

fn parse_days(s: &str) -> Result<Option<DayFilter>> {
    let s = s.trim();
    match s {
        "daily" => Ok(None),
        "weekdays" => Ok(Some(DayFilter::Weekdays)),
        "weekends" => Ok(Some(DayFilter::Weekends)),
        _ => {
            // Try comma-separated day abbreviations
            let days: Result<Vec<DayOfWeek>> = s.split(',').map(|d| parse_day(d.trim())).collect();
            let days = days?;
            if days.is_empty() {
                bail!("no days specified");
            }
            Ok(Some(DayFilter::Days(days)))
        }
    }
}

fn parse_day(s: &str) -> Result<DayOfWeek> {
    match s {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    #[test]
    fn parse_daily_at() {
        let t = parse_trigger("daily at 09:00").unwrap();
        match t {
            TriggerSpec::Recurring { times, days, .. } => {
                assert_eq!(times.len(), 1);
                assert_eq!(times[0].hour, 9);
                assert_eq!(times[0].minute, 0);
                assert!(days.is_none());
            }
            _ => panic!("expected Recurring"),
        }
    }

    #[test]
    fn parse_weekdays_at() {
        let t = parse_trigger("weekdays at 09:30").unwrap();
        match t {
            TriggerSpec::Recurring { days, .. } => {
                assert!(matches!(days, Some(DayFilter::Weekdays)));
            }
            _ => panic!("expected Recurring"),
        }
    }

    #[test]
    fn parse_weekends_at() {
        let t = parse_trigger("weekends at 10:00").unwrap();
        match t {
            TriggerSpec::Recurring { days, .. } => {
                assert!(matches!(days, Some(DayFilter::Weekends)));
            }
            _ => panic!("expected Recurring"),
        }
    }

    #[test]
    fn parse_specific_days() {
        let t = parse_trigger("mon,wed,fri at 14:00").unwrap();
        match t {
            TriggerSpec::Recurring { days, times, .. } => {
                assert_eq!(times[0].hour, 14);
                match days.unwrap() {
                    DayFilter::Days(d) => assert_eq!(d.len(), 3),
                    _ => panic!("expected Days"),
                }
            }
            _ => panic!("expected Recurring"),
        }
    }

    #[test]
    fn parse_every_hours() {
        let t = parse_trigger("every 2 hours").unwrap();
        match t {
            TriggerSpec::Interval { every } => assert_eq!(every.as_secs(), 7200),
            _ => panic!("expected Interval"),
        }
    }

    #[test]
    fn parse_every_minutes() {
        let t = parse_trigger("every 30 minutes").unwrap();
        match t {
            TriggerSpec::Interval { every } => assert_eq!(every.as_secs(), 1800),
            _ => panic!("expected Interval"),
        }
    }

    #[test]
    fn parse_invalid_returns_error() {
        assert!(parse_trigger("banana").is_err());
        assert!(parse_trigger("at 25:00").is_err());
        assert!(parse_trigger("every 0 hours").is_err());
    }
}
