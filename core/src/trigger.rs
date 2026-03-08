use crate::models::{DayFilter, DayOfWeek, TimeOfDay, TriggerSpec};
use anyhow::{Result, bail};
use std::str::FromStr;
use std::time::Duration;

/// Parse a trigger expression string into a [`TriggerSpec`].
///
/// Convenience wrapper around `input.parse::<TriggerSpec>()`.
///
/// # Supported formats
///
/// - `"daily at 09:00"` — every day at the given time
/// - `"weekdays at 09:30"` — Monday through Friday
/// - `"weekends at 10:00"` — Saturday and Sunday
/// - `"mon,wed,fri at 14:00"` — specific days
/// - `"every 2 hours"` — fixed interval (hours, minutes, or seconds)
pub fn parse_trigger(input: &str) -> Result<TriggerSpec> {
    input.parse()
}

/// Parses trigger expressions like `"daily at 09:00"` or `"every 2 hours"`.
///
/// See [`parse_trigger`] for the full list of supported formats.
impl FromStr for TriggerSpec {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let input = s.trim().to_lowercase();

        // "every N unit" pattern
        if let Some(interval) = input.strip_prefix("every ") {
            return parse_interval(interval);
        }

        // "<days> at HH:MM" pattern
        if let Some(at_pos) = input.find(" at ") {
            let days_part = &input[..at_pos];
            let time_part = &input[at_pos + 4..];
            let time: TimeOfDay = time_part.parse()?;
            let days = parse_days(days_part)?;
            return Ok(TriggerSpec::Recurring {
                times: vec![time],
                days,
                timezone: None,
            });
        }

        bail!(
            "unrecognized trigger format: '{input}'. Try 'daily at 09:00', 'weekdays at 09:00', or 'every 2 hours'"
        )
    }
}

fn parse_interval(s: &str) -> Result<TriggerSpec> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        bail!("expected 'every N unit', got 'every {s}'");
    }
    let n: u64 = parts[0]
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid number: '{}'", parts[0]))?;
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

fn parse_days(s: &str) -> Result<Option<DayFilter>> {
    let s = s.trim();
    match s {
        "daily" => Ok(None),
        "weekdays" => Ok(Some(DayFilter::Weekdays)),
        "weekends" => Ok(Some(DayFilter::Weekends)),
        _ => {
            // Try comma-separated day abbreviations
            let days: Result<Vec<DayOfWeek>> =
                s.split(',').map(|d| d.trim().parse::<DayOfWeek>()).collect();
            let days = days?;
            if days.is_empty() {
                bail!("no days specified");
            }
            Ok(Some(DayFilter::Days(days)))
        }
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

    #[test]
    fn trigger_spec_from_str() {
        let t: TriggerSpec = "daily at 09:00".parse().unwrap();
        assert!(matches!(t, TriggerSpec::Recurring { .. }));
    }

}
