use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeOfDay {
    pub hour: u8,
    pub minute: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Frequency of a recurring schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Hourly,
    Daily,
    Weekly,
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

/// Parses `"HH:MM"` (24h) or `"H:MMam"` / `"Hpm"` (12h) time strings,
/// plus `"noon"` and `"midnight"`.
impl FromStr for TimeOfDay {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim().to_lowercase();

        // Special keywords
        if s == "noon" {
            return Ok(TimeOfDay {
                hour: 12,
                minute: 0,
            });
        }
        if s == "midnight" {
            return Ok(TimeOfDay { hour: 0, minute: 0 });
        }

        // Check for am/pm suffix
        let (time_part, period) = if let Some(t) = s.strip_suffix("am") {
            (t.trim(), Some(AmPm::Am))
        } else if let Some(t) = s.strip_suffix("pm") {
            (t.trim(), Some(AmPm::Pm))
        } else {
            (s.as_str(), None)
        };

        let (raw_hour, minute) = if let Some(colon_pos) = time_part.find(':') {
            let hour_str = &time_part[..colon_pos];
            let min_str = &time_part[colon_pos + 1..];
            let hour: u8 = hour_str
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid hour: '{hour_str}'"))?;
            let minute: u8 = min_str
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid minute: '{min_str}'"))?;
            (hour, minute)
        } else if period.is_some() {
            // Bare hour with am/pm, e.g. "8am", "3pm"
            let hour: u8 = time_part
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid hour: '{time_part}'"))?;
            (hour, 0)
        } else {
            bail!("expected time as HH:MM, H:MMam/pm, Ham/pm, noon, or midnight — got '{s}'");
        };

        if minute > 59 {
            bail!("minute must be 0-59, got {minute}");
        }

        let hour = match period {
            Some(AmPm::Am) => {
                if raw_hour == 0 || raw_hour > 12 {
                    bail!(
                        "hour must be 1-12 with am/pm, got {raw_hour}{}",
                        if raw_hour == 0 {
                            " (use 12am for midnight)"
                        } else {
                            ""
                        }
                    );
                }
                if raw_hour == 12 { 0 } else { raw_hour }
            }
            Some(AmPm::Pm) => {
                if raw_hour == 0 || raw_hour > 12 {
                    bail!(
                        "hour must be 1-12 with am/pm, got {raw_hour}{}",
                        if raw_hour == 0 {
                            " (use 12pm for noon)"
                        } else {
                            ""
                        }
                    );
                }
                if raw_hour == 12 { 12 } else { raw_hour + 12 }
            }
            None => {
                if raw_hour > 23 {
                    bail!("hour must be 0-23, got {raw_hour}");
                }
                raw_hour
            }
        };

        Ok(TimeOfDay { hour, minute })
    }
}

enum AmPm {
    Am,
    Pm,
}

/// Parses day-of-week strings (case-insensitive).
///
/// Accepts three-letter abbreviations (`mon`, `tue`, ...) and full names (`monday`, `tuesday`, ...).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_24h() {
        let t: TimeOfDay = "14:30".parse().unwrap();
        assert_eq!(t.hour, 14);
        assert_eq!(t.minute, 30);
    }

    #[test]
    fn time_24h_leading_zero() {
        let t: TimeOfDay = "09:05".parse().unwrap();
        assert_eq!(t.hour, 9);
        assert_eq!(t.minute, 5);
    }

    #[test]
    fn time_12h_with_minutes() {
        let t: TimeOfDay = "8:00am".parse().unwrap();
        assert_eq!(t.hour, 8);
        assert_eq!(t.minute, 0);

        let t: TimeOfDay = "8:00 am".parse().unwrap();
        assert_eq!(t.hour, 8);

        let t: TimeOfDay = "8:00AM".parse().unwrap();
        assert_eq!(t.hour, 8);
    }

    #[test]
    fn time_12h_pm() {
        let t: TimeOfDay = "3:30pm".parse().unwrap();
        assert_eq!(t.hour, 15);
        assert_eq!(t.minute, 30);
    }

    #[test]
    fn time_bare_hour() {
        let t: TimeOfDay = "8am".parse().unwrap();
        assert_eq!(t.hour, 8);
        assert_eq!(t.minute, 0);

        let t: TimeOfDay = "3pm".parse().unwrap();
        assert_eq!(t.hour, 15);
        assert_eq!(t.minute, 0);
    }

    #[test]
    fn time_12am_is_midnight() {
        let t: TimeOfDay = "12am".parse().unwrap();
        assert_eq!(t.hour, 0);
        assert_eq!(t.minute, 0);
    }

    #[test]
    fn time_12pm_is_noon() {
        let t: TimeOfDay = "12pm".parse().unwrap();
        assert_eq!(t.hour, 12);
        assert_eq!(t.minute, 0);
    }

    #[test]
    fn time_noon_midnight_keywords() {
        let t: TimeOfDay = "noon".parse().unwrap();
        assert_eq!(t.hour, 12);
        assert_eq!(t.minute, 0);

        let t: TimeOfDay = "midnight".parse().unwrap();
        assert_eq!(t.hour, 0);
        assert_eq!(t.minute, 0);
    }

    #[test]
    fn time_errors() {
        // 13pm — hour > 12 with am/pm
        assert!("13pm".parse::<TimeOfDay>().is_err());
        // 0am — invalid in 12h
        assert!("0am".parse::<TimeOfDay>().is_err());
        // minute out of range
        assert!("8:60am".parse::<TimeOfDay>().is_err());
        // 24h out of range
        assert!("25:00".parse::<TimeOfDay>().is_err());
        // garbage
        assert!("abc".parse::<TimeOfDay>().is_err());
    }

    #[test]
    fn time_display() {
        let t = TimeOfDay { hour: 9, minute: 5 };
        assert_eq!(t.to_string(), "09:05");
    }

    #[test]
    fn day_of_week_roundtrip() {
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
    fn day_of_week_full_names() {
        assert_eq!("Monday".parse::<DayOfWeek>().unwrap(), DayOfWeek::Monday);
        assert_eq!("FRIDAY".parse::<DayOfWeek>().unwrap(), DayOfWeek::Friday);
        assert!("banana".parse::<DayOfWeek>().is_err());
    }

    #[test]
    fn day_filter_display() {
        assert_eq!(DayFilter::Weekdays.to_string(), "weekdays");
        assert_eq!(DayFilter::Weekends.to_string(), "weekends");
        assert_eq!(
            DayFilter::Days(vec![DayOfWeek::Monday, DayOfWeek::Wednesday]).to_string(),
            "mon,wed"
        );
    }
}
