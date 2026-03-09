use crate::TriggerSpec;
use crate::models::{DayFilter, DayOfWeek, TimeOfDay};
use crate::rrule_convert::{build_interval_rrule, build_recurring_rrule, default_dtstart};
use anyhow::{Result, bail};

/// Parse a trigger expression string into a [`TriggerSpec`].
///
/// # Supported formats
///
/// **Time formats:**
/// - `"8:00am"`, `"8:00 am"`, `"8:00AM"` — 12h with minutes
/// - `"8am"`, `"3pm"` — bare hour (implied :00)
/// - `"noon"` → 12:00, `"midnight"` → 0:00
/// - `"09:30"`, `"14:00"` — 24h format
///
/// **Schedule patterns:**
/// - `"daily at 09:00"` — every day at the given time
/// - `"weekdays at 9am"` — Monday through Friday
/// - `"weekends at 10:00"` — Saturday and Sunday
/// - `"mon,wed,fri at 2pm"` — specific days
/// - `"every 2 hours"` — fixed interval (hours, minutes, or seconds)
pub(crate) fn parse_trigger(input: &str) -> Result<TriggerSpec> {
    let input = input.trim().to_lowercase();

    // "every N unit" pattern
    if let Some(interval) = input.strip_prefix("every ") {
        return parse_interval(interval);
    }

    // "<days> at <time>" pattern
    if let Some(at_pos) = input.find(" at ") {
        let days_part = &input[..at_pos];
        let time_part = &input[at_pos + 4..];
        let time: TimeOfDay = time_part.parse()?;
        let days = parse_days(days_part)?;
        let times = vec![time];
        let rrule = build_recurring_rrule(&times, days.as_ref());
        let dtstart = default_dtstart(&times);
        return Ok(TriggerSpec::RRule {
            rrule,
            dtstart,
            times,
            days,
            interval_secs: None,
        });
    }

    bail!(
        "unrecognized trigger format: '{input}'. Try 'daily at 9am', 'weekdays at 09:00', or 'every 2 hours'"
    )
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
    let rrule = build_interval_rrule(secs);
    let dtstart = default_dtstart(&[]);
    Ok(TriggerSpec::RRule {
        rrule,
        dtstart,
        times: vec![],
        days: None,
        interval_secs: Some(secs),
    })
}

fn parse_days(s: &str) -> Result<Option<DayFilter>> {
    let s = s.trim();
    match s {
        "daily" => Ok(None),
        "weekdays" => Ok(Some(DayFilter::Weekdays)),
        "weekends" => Ok(Some(DayFilter::Weekends)),
        _ => {
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
    use std::time::Duration;

    #[test]
    fn parse_daily_at_24h() {
        let t = parse_trigger("daily at 09:00").unwrap();
        assert_eq!(t.times_of_day()[0].hour, 9);
        assert!(t.days_of_week().is_none());
    }

    #[test]
    fn parse_daily_at_12h() {
        let t = parse_trigger("daily at 9am").unwrap();
        assert_eq!(t.times_of_day()[0].hour, 9);
        assert_eq!(t.times_of_day()[0].minute, 0);
    }

    #[test]
    fn parse_weekdays_at_12h() {
        let t = parse_trigger("weekdays at 9am").unwrap();
        assert_eq!(t.times_of_day()[0].hour, 9);
        assert!(matches!(t.days_of_week(), Some(days) if days.len() == 5));
        assert!(t.to_rrule_string().contains("BYDAY=MO,TU,WE,TH,FR"));
    }

    #[test]
    fn parse_weekends() {
        let t = parse_trigger("weekends at 10:00").unwrap();
        assert!(t.to_rrule_string().contains("BYDAY=SA,SU"));
    }

    #[test]
    fn parse_specific_days_12h() {
        let t = parse_trigger("mon,wed,fri at 2pm").unwrap();
        assert_eq!(t.times_of_day()[0].hour, 14);
        assert!(t.to_rrule_string().contains("BYDAY=MO,WE,FR"));
    }

    #[test]
    fn parse_every_hours() {
        let t = parse_trigger("every 2 hours").unwrap();
        assert_eq!(t.interval(), Some(Duration::from_secs(7200)));
    }

    #[test]
    fn parse_every_minutes() {
        let t = parse_trigger("every 30 minutes").unwrap();
        assert_eq!(t.interval(), Some(Duration::from_secs(1800)));
    }

    #[test]
    fn parse_noon() {
        let t = parse_trigger("daily at noon").unwrap();
        assert_eq!(t.times_of_day()[0].hour, 12);
        assert_eq!(t.times_of_day()[0].minute, 0);
    }

    #[test]
    fn parse_midnight() {
        let t = parse_trigger("daily at midnight").unwrap();
        assert_eq!(t.times_of_day()[0].hour, 0);
    }

    #[test]
    fn parse_errors() {
        assert!(parse_trigger("banana").is_err());
        assert!(parse_trigger("every 0 hours").is_err());
    }
}
