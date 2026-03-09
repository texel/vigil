use crate::models::{DayFilter, DayOfWeek, TimeOfDay};

/// Build an RRULE string for a recurring schedule.
pub(crate) fn build_recurring_rrule(
    times: &[TimeOfDay],
    days: Option<&DayFilter>,
) -> String {
    let mut parts = Vec::new();

    match days {
        Some(DayFilter::Weekdays) => {
            parts.push("FREQ=WEEKLY".to_string());
            parts.push("BYDAY=MO,TU,WE,TH,FR".to_string());
        }
        Some(DayFilter::Weekends) => {
            parts.push("FREQ=WEEKLY".to_string());
            parts.push("BYDAY=SA,SU".to_string());
        }
        Some(DayFilter::Days(day_list)) => {
            parts.push("FREQ=WEEKLY".to_string());
            let byday: Vec<&str> = day_list.iter().map(day_to_rrule).collect();
            parts.push(format!("BYDAY={}", byday.join(",")));
        }
        None => {
            parts.push("FREQ=DAILY".to_string());
        }
    }

    // Encode times as BYHOUR/BYMINUTE if multiple, otherwise we rely on DTSTART
    if times.len() > 1 {
        let hours: Vec<String> = times.iter().map(|t| t.hour.to_string()).collect();
        let minutes: Vec<String> = times.iter().map(|t| t.minute.to_string()).collect();
        parts.push(format!("BYHOUR={}", hours.join(",")));
        parts.push(format!("BYMINUTE={}", minutes.join(",")));
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
