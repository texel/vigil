use chrono::TimeDelta;
use vigil_core::models::{Run, RunSummary};

pub fn render_markdown(text: &str) {
    let skin = termimad::MadSkin::default();
    skin.print_text(text);
}

pub fn print_run_header(task_name: &str, run: &Run, summary: &RunSummary) {
    println!("Task:    {task_name}");
    println!("Run:     {}", &run.id.to_string()[..8]);
    println!("Status:  {}", run.status);
    if let Some(completed) = run.completed_at {
        let dur = completed - run.started_at;
        println!("Duration: {}", format_duration(dur));
    }
    for (key, value) in &summary.fields {
        println!("{key}: {value}");
    }
    println!("{}", "\u{2500}".repeat(60));
}

pub fn format_duration(dur: TimeDelta) -> String {
    let total_secs = dur.num_milliseconds() as f64 / 1000.0;
    if total_secs < 60.0 {
        format!("{total_secs:.1}s")
    } else if total_secs < 3600.0 {
        format!(
            "{:.0}m {:.0}s",
            (total_secs / 60.0).floor(),
            total_secs % 60.0
        )
    } else {
        let hours = (total_secs / 3600.0).floor();
        let mins = ((total_secs % 3600.0) / 60.0).floor();
        format!("{hours:.0}h {mins:.0}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(TimeDelta::milliseconds(500)), "0.5s");
        assert_eq!(format_duration(TimeDelta::milliseconds(5000)), "5.0s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(TimeDelta::milliseconds(150_000)), "2m 30s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(TimeDelta::milliseconds(4_500_000)), "1h 15m");
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(TimeDelta::milliseconds(0)), "0.0s");
    }
}
