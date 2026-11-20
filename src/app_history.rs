//! Per-app energy use over time, kept in hourly buckets for 7 days, so the
//! Usage page can show which apps used the most over 24 hours or a week.
//!
//! For each app and hour this stores CPU-seconds (the same measure as energy
//! impact) and estimated watt-hours drawn on battery.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::apps::AppUsage;
use crate::history::data_dir;

const CHECKPOINT_SECS: i64 = 60;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppTotal {
    pub key: String,
    pub name: String,
    pub cpu_secs: f64,
    pub wh: f64,
}

pub struct AppHistory {
    path: PathBuf,
    pub write_error: Option<String>,
    dirty: bool,
    writer: bool,
    last_checkpoint: Option<i64>,
    retention_days: u16,
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
        Self::from_path(data_dir().join("app-history.csv"), now, crate::settings::Settings::load().app_history_days)
    }

    fn from_path(path: PathBuf, now: i64, retention_days: u16) -> Self {
        let mut h = Self { path, write_error: None, dirty: false, writer: false, last_checkpoint: None,
            retention_days, buckets: HashMap::new(), current_hour: hour_of(now), current: HashMap::new() };
        for (hour, total) in fs::read_to_string(&h.path).unwrap_or_default().lines().filter_map(parse_line) {
            if retention_days == 0 || hour + 3600 > now - i64::from(retention_days) * 86_400 {
                if hour == h.current_hour { h.current.insert(total.key.clone(), total); }
                else { h.buckets.insert((hour, total.key.clone()), total); }
            }
        }
        h
    }

    /// Checkpoint the current hour every minute, as well as at hour boundaries.
    pub fn add(&mut self, apps: &[AppUsage], secs: f64, now: i64, write: bool) {
        self.writer = write;
        let changed_hour = hour_of(now) != self.current_hour;
        if changed_hour {
            for (key, total) in self.current.drain() { self.buckets.insert((self.current_hour, key), total); }
            self.current_hour = hour_of(now);
        }
        for app in apps.iter().filter(|a| a.impact.is_finite() && a.impact > 0.0) {
            let t = self.current.entry(app.key.clone()).or_insert_with(|| AppTotal {
                key: app.key.clone(), name: app.name.clone(), ..Default::default()
            });
            t.cpu_secs += app.impact / 100.0 * secs.max(0.0);
            t.wh += app.watts.filter(|w| w.is_finite() && *w >= 0.0).unwrap_or(0.0) * secs.max(0.0) / 3600.0;
            self.dirty = true;
        }
        if changed_hour { self.prune(now, self.retention_days); }
        if write && (changed_hour || self.last_checkpoint.is_none_or(|t| now - t >= CHECKPOINT_SECS) || self.write_error.is_some()) {
            self.checkpoint(now);
        }
    }

    pub fn record_count(&self) -> usize { self.buckets.len() + self.current.len() }

    pub fn prune(&mut self, now: i64, days: u16) {
        self.retention_days = days;
        let before = self.buckets.len();
        self.buckets.retain(|(hour, _), _| days == 0 || *hour + 3600 > now - i64::from(days) * 86_400);
        self.dirty |= self.buckets.len() != before;
    }

    /// Atomic replacement makes checkpoints restart-safe and avoids duplicate hours.
    pub fn checkpoint(&mut self, now: i64) {
        if !self.dirty { return; }
        let mut rows: Vec<_> = self.buckets.iter().map(|((hour, _), t)| (*hour, t)).chain(self.current.values().map(|t| (self.current_hour, t))).collect();
        rows.sort_by(|a, b| (a.0, &a.1.key).cmp(&(b.0, &b.1.key)));
        let text: String = rows.into_iter().map(|(hour, t)| format!("{hour},{},{:.3},{:.6},{}\n", t.key.replace([',', '\n', '\r'], "_"), t.cpu_secs, t.wh, t.name.replace(['\n', '\r'], " "))).collect();
        self.write_error = crate::storage::replace(&self.path, &text).err();
        if self.write_error.is_none() { self.dirty = false; self.last_checkpoint = Some(now); }
    }

    pub fn flush(&mut self) { if self.writer { self.checkpoint(crate::history::now_unix()); } }

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
        let mut rows: Vec<(i64, AppTotal)> = self.buckets.iter().map(|((h, _), t)| (*h, t.clone())).chain(self.current.values().map(|t| (self.current_hour, t.clone()))).collect();
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
    fn checkpoints_resume_current_hour_and_flush_without_duplicates() {
        let dir = std::env::temp_dir().join(format!("bhc-checkpoints-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("apps.csv");
        let mut h = AppHistory::from_path(path.clone(), 10, 7);
        h.add(&[usage("a", 100.0)], 10.0, 10, true);
        h.add(&[usage("a", 50.0)], 20.0, 30, true);
        let checkpoint = AppHistory::from_path(path.clone(), 31, 7);
        assert_eq!(checkpoint.totals(0)[0].cpu_secs, 10.0);
        h.flush();
        let mut resumed = AppHistory::from_path(path.clone(), 40, 7);
        assert_eq!(resumed.totals(0)[0].cpu_secs, 20.0);
        resumed.add(&[usage("a", 100.0)], 5.0, 40, true);
        resumed.checkpoint(41);
        assert_eq!(AppHistory::from_path(path.clone(), 42, 7).totals(0)[0].cpu_secs, 25.0);
        assert_eq!(fs::read_to_string(path).unwrap().lines().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn retention_prunes_persisted_app_hours() {
        let dir = std::env::temp_dir().join(format!("bhc-app-retention-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap(); let path = dir.join("apps.csv");
        let mut h = AppHistory::from_path(path.clone(), 0, 0);
        h.add(&[usage("a", 100.0)], 10.0, 0, true);
        h.add(&[usage("b", 100.0)], 10.0, 10 * 86_400, true);
        h.prune(10 * 86_400, 7); h.flush();
        let text = fs::read_to_string(path).unwrap(); assert!(!text.contains(",a,")); assert!(text.contains(",b,"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_hour_remains_pending_and_retries_without_new_hour() {
        let dir = std::env::temp_dir().join(format!("bhc-app-errors-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let mut h = AppHistory::from_path(blocker.join("apps.csv"), 0, 7);
        h.add(&[usage("a", 50.0)], 10.0, 0, true);
        h.add(&[], 1.0, 3600, true);
        assert!(h.write_error.is_some());
        assert!(h.dirty);
        h.path = dir.join("apps.csv");
        h.add(&[], 1.0, 3601, true);
        assert!(h.write_error.is_none());
        assert!(!h.dirty);
        assert_eq!(fs::read_to_string(&h.path).unwrap().lines().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn series_has_a_point_for_every_recorded_hour() {
        let mut h = AppHistory::from_path(PathBuf::new(), 0, 7);
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
        assert_eq!(h.hourly().len(), 4, "includes the current hour");
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
