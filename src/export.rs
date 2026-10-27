//! Exports the recorded history as CSV files, for spreadsheets or other tools:
//! battery level and power draw, capacity over time, sleep sessions, and each
//! app's energy use per hour. Times are written twice, as local ISO 8601 and as
//! Unix seconds, and statuses use fixed English names, so the files read the
//! same in any language.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::battery::Status;
use crate::controller::State;

/// A CSV field, quoted when it holds a comma, a quote or a line break.
fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Charging => "charging",
        Status::Discharging => "discharging",
        Status::NotCharging => "not_charging",
        Status::Full => "full",
        Status::Unknown => "unknown",
    }
}

fn opt(v: Option<f64>, decimals: usize) -> String {
    v.map(|v| format!("{v:.decimals$}")).unwrap_or_default()
}

pub fn battery_csv(st: &State, iso: &dyn Fn(i64) -> String) -> String {
    let mut out = String::from("time,unix_time,percent,status,power_w,max_capacity_percent\n");
    for s in &st.history.samples {
        out.push_str(&format!("{},{},{:.1},{},{},{}\n", iso(s.ts), s.ts, s.percent, status_name(s.status), opt(s.power_w, 2), opt(s.health, 1)));
    }
    out
}

pub fn capacity_csv(st: &State, iso: &dyn Fn(i64) -> String) -> String {
    let mut out = String::from("time,unix_time,max_capacity_percent,cycles\n");
    for e in &st.health_log.entries {
        out.push_str(&format!("{},{},{:.1},{}\n", iso(e.ts), e.ts, e.health, e.cycles.map(|c| c.to_string()).unwrap_or_default()));
    }
    out
}

pub fn sleep_csv(st: &State, iso: &dyn Fn(i64) -> String) -> String {
    let mut out = String::from("start,unix_time,asleep_minutes,percent_before,percent_after,percent_per_hour\n");
    for s in st.history.sleep_sessions() {
        out.push_str(&format!(
            "{},{},{:.0},{:.0},{:.0},{:.2}\n",
            iso(s.start_ts),
            s.start_ts,
            s.slept_secs as f64 / 60.0,
            s.from_percent,
            s.to_percent,
            s.percent_per_hour()
        ));
    }
    out
}

pub fn app_energy_csv(st: &State, iso: &dyn Fn(i64) -> String) -> String {
    let mut out = String::from("hour_start,unix_time,app,cpu_seconds,energy_wh_on_battery\n");
    for (hour, t) in st.app_history.hourly() {
        out.push_str(&format!("{},{},{},{:.1},{:.4}\n", iso(hour), hour, field(&t.name), t.cpu_secs, t.wh));
    }
    out
}

/// Writes the CSV files into `dir` and returns their paths.
pub fn write_all(dir: &Path, st: &State, iso: &dyn Fn(i64) -> String) -> io::Result<Vec<PathBuf>> {
    let files = [
        ("battery-history.csv", battery_csv(st, iso)),
        ("capacity-history.csv", capacity_csv(st, iso)),
        ("sleep-sessions.csv", sleep_csv(st, iso)),
        ("app-energy.csv", app_energy_csv(st, iso)),
    ];
    let mut written = Vec::new();
    for (name, text) in files {
        let path = dir.join(name);
        fs::write(&path, text)?;
        written.push(path);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_quoted_when_needed() {
        assert_eq!(field("Plain"), "Plain");
        assert_eq!(field("Foo, the app"), "\"Foo, the app\"");
        assert_eq!(field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(field("two\nlines"), "\"two\nlines\"");
    }

    #[test]
    fn statuses_have_fixed_names() {
        assert_eq!(status_name(Status::NotCharging), "not_charging");
        assert_eq!(status_name(Status::Discharging), "discharging");
    }

    #[test]
    fn optional_numbers() {
        assert_eq!(opt(Some(12.345), 2), "12.35");
        assert_eq!(opt(None, 2), "");
    }
}
