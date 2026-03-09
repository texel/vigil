use crate::models::{DayFilter, DayOfWeek, TimeOfDay};

/// Synthesize a DTSTART string for schedules that don't have one stored.
///
/// Uses a fixed past date (2020-01-01, a Wednesday) with the first scheduled time,
/// or midnight if no times are specified. This ensures RRULE evaluation works for
/// legacy DB rows that predate the `dtstart` field.
pub(crate) fn default_dtstart(times: &[TimeOfDay]) -> String {
    match times.first() {
        Some(t) => format!("20200101T{:02}{:02}00Z", t.hour, t.minute),
        None => "20200101T000000Z".to_string(),
    }
}

/// Build an RRULE string for a recurring schedule.
pub(crate) fn build_recurring_rrule(
    times: &[TimeOfDay],
    days: Option<&DayFilter>,
) -> String {
    let mut parts: Vec<&str> = Vec::new();

    let byday_str;
    match days {
        Some(DayFilter::Weekdays) => {
            parts.push("FREQ=WEEKLY");
            parts.push("BYDAY=MO,TU,WE,TH,FR");
        }
        Some(DayFilter::Weekends) => {
            parts.push("FREQ=WEEKLY");
            parts.push("BYDAY=SA,SU");
        }
        Some(DayFilter::Days(day_list)) => {
            parts.push("FREQ=WEEKLY");
            let byday: Vec<&str> = day_list.iter().map(day_to_rrule).collect();
            byday_str = format!("BYDAY={}", byday.join(","));
            parts.push(&byday_str);
        }
        None => {
            parts.push("FREQ=DAILY");
        }
    }

    // Encode times as BYHOUR/BYMINUTE if multiple, otherwise we rely on DTSTART
    let byhour_str;
    let byminute_str;
    if times.len() > 1 {
        let hours: Vec<String> = times.iter().map(|t| t.hour.to_string()).collect();
        let minutes: Vec<String> = times.iter().map(|t| t.minute.to_string()).collect();
        byhour_str = format!("BYHOUR={}", hours.join(","));
        byminute_str = format!("BYMINUTE={}", minutes.join(","));
        parts.push(&byhour_str);
        parts.push(&byminute_str);
    }

    format!("RRULE:{}", parts.join(";"))
}

/// Build an RRULE string for an interval schedule.
pub(crate) fn build_interval_rrule(seconds: u64) -> String {
    if seconds % 3600 == 0 {
        let hours = seconds / 3600;
        format!("RRULE:FREQ=HOURLY;INTERVAL={hours}")
    } else if seconds % 60 == 0 {
        let mins = seconds / 60;
        format!("RRULE:FREQ=MINUTELY;INTERVAL={mins}")
    } else {
        format!("RRULE:FREQ=SECONDLY;INTERVAL={seconds}")
    }
}

fn day_to_rrule(day: &DayOfWeek) -> &'static str {
    match day {
        DayOfWeek::Monday => "MO",
        DayOfWeek::Tuesday => "TU",
        DayOfWeek::Wednesday => "WE",
        DayOfWeek::Thursday => "TH",
        DayOfWeek::Friday => "FR",
        DayOfWeek::Saturday => "SA",
        DayOfWeek::Sunday => "SU",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurring_daily() {
        let rrule = build_recurring_rrule(
            &[TimeOfDay { hour: 9, minute: 0 }],
            None,
        );
        assert_eq!(rrule, "RRULE:FREQ=DAILY");
    }

    #[test]
    fn recurring_weekdays() {
        let rrule = build_recurring_rrule(
            &[TimeOfDay { hour: 9, minute: 0 }],
            Some(&DayFilter::Weekdays),
        );
        assert_eq!(rrule, "RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR");
    }

    #[test]
    fn recurring_weekends() {
        let rrule = build_recurring_rrule(
            &[TimeOfDay { hour: 10, minute: 0 }],
            Some(&DayFilter::Weekends),
        );
        assert_eq!(rrule, "RRULE:FREQ=WEEKLY;BYDAY=SA,SU");
    }

    #[test]
    fn recurring_specific_days() {
        let rrule = build_recurring_rrule(
            &[TimeOfDay { hour: 14, minute: 0 }],
            Some(&DayFilter::Days(vec![
                DayOfWeek::Monday,
                DayOfWeek::Wednesday,
                DayOfWeek::Friday,
            ])),
        );
        assert_eq!(rrule, "RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR");
    }

    #[test]
    fn interval_hours() {
        assert_eq!(build_interval_rrule(7200), "RRULE:FREQ=HOURLY;INTERVAL=2");
    }

    #[test]
    fn interval_minutes() {
        assert_eq!(build_interval_rrule(1800), "RRULE:FREQ=MINUTELY;INTERVAL=30");
    }

    #[test]
    fn interval_seconds() {
        assert_eq!(build_interval_rrule(45), "RRULE:FREQ=SECONDLY;INTERVAL=45");
    }
}
