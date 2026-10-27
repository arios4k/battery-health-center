//! Per-app energy use over time, kept in hourly buckets for 7 days, so the
//! Usage page can show which apps used the most over 24 hours or a week.
//!
//! For each app and hour this stores CPU-seconds (the same measure as energy
//! impact) and estimated watt-hours drawn on battery.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::apps::AppUsage;
use crate::history::data_dir;

const RETENTION_SECS: i64 = 7 * 86_400;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppTotal {
    pub key: String,
    pub name: String,
    pub cpu_secs: f64,
    pub wh: f64,
}

pub struct AppHistory {
    path: PathBuf,
    /// Completed hours, keyed by (hour start, app key).
    buckets: HashMap<(i64, String), AppTotal>,
    /// The hour currently being accumulated.
    current_hour: i64,
    current: HashMap<String, AppTotal>,
}

fn hour_of(ts: i64) -> i64 {
    ts - ts.rem_euclid(3600)
}

fn parse_line(line: &str) -> Option<(i64, AppTotal)> {
    let mut f = line.splitn(5, ',');
    let hour = f.next()?.parse().ok()?;
    let key = f.next()?.to_string();
    let cpu_secs = f.next()?.parse().ok()?;
    let wh = f.next()?.parse().ok()?;
    let name = f.next()?.to_string();
    Some((hour, AppTotal { key, name, cpu_secs, wh }))
}

impl AppHistory {
    pub fn load(now: i64) -> Self {
        let path = data_dir().join("app-history.csv");
        let cutoff = now - RETENTION_SECS;
        let mut buckets = HashMap::new();
        for (hour, total) in fs::read_to_string(&path).unwrap_or_default().lines().filter_map(parse_line) {
            if hour >= cutoff {
                buckets.insert((hour, total.key.clone()), total);
            }
        }
        AppHistory { path, buckets, current_hour: hour_of(now), current: HashMap::new() }
    }

    /// Adds `secs` seconds of the given usage. When an hour ends, its totals
    /// are written to disk if `write` is set (only the history writer does).
    pub fn add(&mut self, apps: &[AppUsage], secs: f64, now: i64, write: bool) {
        if hour_of(now) != self.current_hour {
            self.close_hour(write, now);
        }
        for app in apps.iter().filter(|a| a.impact > 0.0) {
            let t = self.current.entry(app.key.clone()).or_insert_with(|| AppTotal {
                key: app.key.clone(),
                name: app.name.clone(),
                ..Default::default()
            });
            t.cpu_secs += app.impact / 100.0 * secs;
            t.wh += app.watts.unwrap_or(0.0) * secs / 3600.0;
        }
    }

    fn close_hour(&mut self, write: bool, now: i64) {
        let hour = self.current_hour;
        let finished: Vec<AppTotal> = self.current.drain().map(|(_, t)| t).collect();
        if write && !finished.is_empty() {
            if let Some(dir) = self.path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
                for t in &finished {
                    // The name goes last so commas in it don't break parsing.
                    let _ = writeln!(f, "{hour},{},{:.1},{:.4},{}", t.key, t.cpu_secs, t.wh, t.name.replace('\n', " "));
                }
            }
        }
        for t in finished {
            self.buckets.insert((hour, t.key.clone()), t);
        }
        self.current_hour = hour_of(now);
        let cutoff = now - RETENTION_SECS;
        self.buckets.retain(|(h, _), _| *h >= cutoff);
    }

    /// Totals per app since `since`, including the hour in progress, sorted
    /// by CPU time.
    pub fn totals(&self, since: i64) -> Vec<AppTotal> {
        let mut sum: HashMap<&str, AppTotal> = HashMap::new();
        let recent = self.buckets.iter().filter(|((h, _), _)| *h + 3600 > since).map(|(_, t)| t);
        for t in recent.chain(self.current.values()) {
            let e = sum.entry(&t.key).or_insert_with(|| AppTotal { key: t.key.clone(), name: t.name.clone(), ..Default::default() });
            e.cpu_secs += t.cpu_secs;
            e.wh += t.wh;
        }
        let mut out: Vec<AppTotal> = sum.into_values().collect();
        out.sort_by(|a, b| b.cpu_secs.total_cmp(&a.cpu_secs));
        out
    }
}

/// One hour of one app's use, for the trend chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HourPoint {
    pub hour: i64,
    /// Seconds of that hour that have passed (less than an hour for the one in progress).
    pub covered_secs: f64,
    pub cpu_secs: f64,
    pub wh: f64,
}

impl HourPoint {
    /// Average energy impact over the hour, in percent of one core.
    pub fn impact(&self) -> f64 {
        self.cpu_secs / self.covered_secs.max(1.0) * 100.0
    }
}

impl AppHistory {
    /// An app's use hour by hour since `since`. Every hour in which anything was
    /// recorded gets a point (zero if this app did nothing), so quiet hours show
    /// as quiet instead of as gaps.
    pub fn series(&self, key: &str, since: i64, now: i64) -> Vec<HourPoint> {
        let mut hours: HashMap<i64, HourPoint> = HashMap::new();
        let first = hour_of(since);
        let done = self.buckets.iter().filter(|((h, _), _)| *h >= first).map(|((h, _), t)| (*h, t));
        let current = self.current.values().map(|t| (self.current_hour, t));
        for (hour, t) in done.chain(current) {
            let covered = if hour == self.current_hour { (now - hour).clamp(1, 3600) as f64 } else { 3600.0 };
            let p = hours.entry(hour).or_insert(HourPoint { hour, covered_secs: covered, cpu_secs: 0.0, wh: 0.0 });
            if t.key == key {
                p.cpu_secs += t.cpu_secs;
                p.wh += t.wh;
            }
        }
        let mut out: Vec<HourPoint> = hours.into_values().collect();
        out.sort_by_key(|p| p.hour);
        out
    }

    /// Every stored hour and app, oldest first, for exporting.
    pub fn hourly(&self) -> Vec<(i64, AppTotal)> {
        let mut rows: Vec<(i64, AppTotal)> = self.buckets.iter().map(|((h, _), t)| (*h, t.clone())).collect();
        rows.sort_by(|a, b| (a.0, &a.1.name).cmp(&(b.0, &b.1.name)));
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(key: &str, impact: f64) -> AppUsage {
        AppUsage { key: key.into(), name: key.into(), icon: None, system: false, processes: 1, impact, watts: Some(2.0) }
    }

    #[test]
    fn series_has_a_point_for_every_recorded_hour() {
        let mut h = AppHistory { path: PathBuf::new(), buckets: HashMap::new(), current_hour: 0, current: HashMap::new() };
        // Hour 0: both apps, hour 1: only "b", hour 2 (in progress): only "a".
        h.add(&[usage("a", 50.0), usage("b", 10.0)], 3600.0, 10, false);
        h.add(&[usage("b", 10.0)], 3600.0, 3600 + 10, false);
        h.add(&[usage("a", 20.0)], 1800.0, 7200 + 1800, false);
        let a = h.series("a", 0, 7200 + 1800);
        assert_eq!(a.iter().map(|p| p.hour).collect::<Vec<_>>(), vec![0, 3600, 7200]);
        assert_eq!(a[0].impact().round(), 50.0);
        assert_eq!(a[1].cpu_secs, 0.0, "a did nothing in hour 1");
        // The hour in progress is judged over the part that has passed.
        assert_eq!(a[2].covered_secs, 1800.0);
        assert_eq!(a[2].impact().round(), 20.0);
        assert!(h.series("a", 3600, 7200 + 1800).iter().all(|p| p.hour >= 3600));
        assert_eq!(h.hourly().len(), 3, "completed hours only");
    }

    #[test]
    fn line_roundtrip_with_comma_in_name() {
        let (hour, t) = parse_line("3600,app:org.foo,12.5,0.2500,Foo, the app").unwrap();
        assert_eq!(hour, 3600);
        assert_eq!(t.key, "app:org.foo");
        assert_eq!(t.name, "Foo, the app");
        assert_eq!(t.wh, 0.25);
    }
}
