//! One battery health entry per day, kept indefinitely, for long-term trends
//! (the main history only keeps 30 days). Stored as CSV next to the history.

use std::fs;
use std::path::PathBuf;

use crate::battery::Battery;
use crate::history::data_dir;

#[derive(Clone, Debug, PartialEq)]
pub struct HealthEntry {
    pub ts: i64,
    pub health: f64,
    pub cycles: Option<u32>,
    pub battery_id: String,
    pub battery_label: String,
}

/// Capacity trend from a linear fit over the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Trend {
    /// Change in health percentage points per 30 days (negative = losing).
    pub per_month: f64,
    /// When health is projected to reach 80%, if it's declining and above 80%.
    pub reaches_80_ts: Option<i64>,
    pub span_days: f64,
}

/// A trend needs at least this much history to mean anything.
const MIN_TREND_DAYS: f64 = 14.0;
const MIN_TREND_POINTS: usize = 5;

pub struct HealthLog {
    path: PathBuf,
    pub entries: Vec<HealthEntry>,
    pub write_error: Option<String>,
}

fn parse_line(line: &str) -> Option<HealthEntry> {
    let mut f = line.splitn(5, ',');
    Some(HealthEntry {
        ts: f.next()?.parse().ok()?,
        health: f.next()?.parse().ok()?,
        cycles: f.next().and_then(|c| c.parse().ok()),
        battery_id: f.next().unwrap_or("legacy").to_string(),
        battery_label: f.next().unwrap_or("legacy").to_string(),
    })
}

impl HealthLog {
    pub fn load() -> Self {
        let path = data_dir().join("health.csv");
        let entries = fs::read_to_string(&path).unwrap_or_default().lines().filter_map(parse_line).collect();
        HealthLog { path, entries, write_error: None }
    }

    /// Adds today's entry if there isn't one yet. Only the process that writes
    /// history calls this, so entries aren't duplicated.
    pub fn record(&mut self, battery: &Battery, now: i64) -> bool {
        let Some(health) = battery.health_percent() else { return false };
        let day = |ts: i64| ts.div_euclid(86_400);
        if self.entries.iter().rev().any(|e| e.battery_id == battery.identity() && day(e.ts) == day(now)) {
            return false;
        }
        let entry = HealthEntry { ts: now, health, cycles: battery.cycle_count, battery_id: battery.identity(),
            battery_label: format!("{} · {}", battery.name, battery.model.as_deref().unwrap_or("")).replace(['\n', '\r'], " ") };
        let cycles = entry.cycles.map(|c| c.to_string()).unwrap_or_default();
        self.write_error = crate::storage::append(&self.path, &format!("{},{:.2},{},{},{}\n", entry.ts, entry.health, cycles, entry.battery_id, entry.battery_label)).err();
        if self.write_error.is_some() { return false; } // retry on the next tick
        self.entries.push(entry);
        true
    }

    pub fn entries_for(&self, id: &str) -> Vec<HealthEntry> {
        self.entries.iter().filter(|e| e.battery_id == id).cloned().collect()
    }

    pub fn trend_for(&self, id: &str) -> Option<Trend> { trend(&self.entries_for(id)) }

    pub fn prune(&mut self, now: i64, days: u16, write: bool) {
        if days == 0 { return; }
        let retained: Vec<_> = self.entries.iter().filter(|e| e.ts >= now - i64::from(days) * 86_400).cloned().collect();
        if retained.len() == self.entries.len() { return; }
        if write {
            let text: String = retained.iter().map(|e| format!("{},{:.2},{},{},{}\n", e.ts, e.health,
                e.cycles.map(|c| c.to_string()).unwrap_or_default(), e.battery_id, e.battery_label)).collect();
            self.write_error = crate::storage::replace(&self.path, &text).err();
            if self.write_error.is_some() { return; }
        }
        self.entries = retained;
    }

}

fn trend(entries: &[HealthEntry]) -> Option<Trend> {
    let (first, last) = (entries.first()?, entries.last()?);
    let span_days = (last.ts - first.ts) as f64 / 86_400.0;
    if span_days < MIN_TREND_DAYS || entries.len() < MIN_TREND_POINTS {
        return None;
    }
    // Least-squares fit of health against time (in days since the first entry).
    let n = entries.len() as f64;
    let xs: Vec<f64> = entries.iter().map(|e| (e.ts - first.ts) as f64 / 86_400.0).collect();
    let (mx, my) = (xs.iter().sum::<f64>() / n, entries.iter().map(|e| e.health).sum::<f64>() / n);
    let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
    let sxy: f64 = xs.iter().zip(entries).map(|(x, e)| (x - mx) * (e.health - my)).sum();
    if sxx == 0.0 {
        return None;
    }
    let per_day = sxy / sxx;
    let now_fit = my + per_day * (xs[xs.len() - 1] - mx);
    let reaches_80_ts = (per_day < 0.0 && now_fit > 80.0)
        .then(|| last.ts + ((now_fit - 80.0) / -per_day * 86_400.0) as i64);
    Some(Trend { per_month: per_day * 30.0, reaches_80_ts, span_days })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_daily_entry_is_retried_after_storage_recovers() {
        let dir = std::env::temp_dir().join(format!("bhc-health-errors-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("energy_full"), "40000000").unwrap();
        fs::write(dir.join("energy_full_design"), "50000000").unwrap();
        let battery = crate::battery::read_battery(&dir).unwrap();
        let blocker = dir.join("blocker");
        fs::write(&blocker, "file").unwrap();
        let mut log = HealthLog { path: blocker.join("health.csv"), entries: Vec::new(), write_error: None };
        assert!(!log.record(&battery, 100));
        assert!(log.entries.is_empty());
        assert!(log.write_error.is_some());
        log.path = dir.join("health.csv");
        assert!(log.record(&battery, 100));
        assert!(log.write_error.is_none());
        assert!(!log.record(&battery, 101));
        assert_eq!(fs::read_to_string(&log.path).unwrap().lines().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replacement_batteries_and_legacy_records_never_share_a_trend() {
        let dir = std::env::temp_dir().join(format!("bhc-health-identities-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("energy_full"), "40000000").unwrap();
        fs::write(dir.join("energy_full_design"), "50000000").unwrap();
        fs::write(dir.join("serial_number"), "original").unwrap();
        let old = crate::battery::read_battery(&dir).unwrap();
        fs::write(dir.join("serial_number"), "replacement").unwrap();
        let new = crate::battery::read_battery(&dir).unwrap();
        assert_ne!(old.identity(), new.identity());
        let legacy = parse_line("1,70,200").unwrap(); assert_eq!(legacy.battery_id, "legacy");
        let mut log = HealthLog { path: dir.join("health.csv"), entries: vec![legacy], write_error: None };
        for day in 0..20 { log.record(&old, day * 86_400 + 100); }
        assert!(log.trend_for(&old.identity()).is_some());
        assert!(log.record(&new, 19 * 86_400 + 100));
        assert_eq!(log.entries_for(&new.identity()).len(), 1);
        assert!(log.trend_for(&new.identity()).is_none());
        assert_eq!(log.entries_for("legacy").len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn declining_trend() {
        // Losing 0.1 points per day from 90%: 80% is 100 days after day 0.
        let entries: Vec<HealthEntry> = (0..30)
            .map(|d| HealthEntry { ts: d * 86_400, health: 90.0 - 0.1 * d as f64, cycles: None, battery_id: "legacy".into(), battery_label: "legacy".into() })
            .collect();
        let t = trend(&entries).unwrap();
        assert!((t.per_month + 3.0).abs() < 1e-6);
        let days_to_80 = t.reaches_80_ts.unwrap() as f64 / 86_400.0;
        assert!((days_to_80 - 100.0).abs() < 0.01);
    }

    #[test]
    fn too_little_data() {
        let entries: Vec<HealthEntry> =
            (0..5).map(|d| HealthEntry { ts: d * 86_400, health: 90.0, cycles: None, battery_id: "legacy".into(), battery_label: "legacy".into() }).collect();
        assert!(trend(&entries).is_none());
    }
}
