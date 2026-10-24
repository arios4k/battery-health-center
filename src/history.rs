//! Battery level history, stored as CSV in ~/.local/share/battery-health-center.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::battery::{Battery, Status};

/// How long samples are kept on disk.
const RETENTION_SECS: i64 = 30 * 24 * 3600;

#[derive(Clone, Debug)]
pub struct Sample {
    pub ts: i64,
    pub percent: f64,
    pub status: Status,
    pub power_w: Option<f64>,
    pub health: Option<f64>,
}

impl Sample {
    /// Plugged in (charging, full, or held at the charge limit).
    pub fn on_power(&self) -> bool {
        matches!(self.status, Status::Charging | Status::Full | Status::NotCharging)
    }
}

pub struct History {
    path: PathBuf,
    pub samples: Vec<Sample>,
}

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn parse_line(line: &str) -> Option<Sample> {
    let mut f = line.split(',');
    let ts = f.next()?.parse().ok()?;
    let percent = f.next()?.parse().ok()?;
    let status = Status::from_code(f.next()?.chars().next()?);
    let power_w = f.next().and_then(|s| s.parse().ok());
    let health = f.next().and_then(|s| s.parse().ok());
    Some(Sample { ts, percent, status, power_w, health })
}

fn format_line(s: &Sample) -> String {
    let opt = |v: Option<f64>| v.map(|v| format!("{v:.2}")).unwrap_or_default();
    format!("{},{:.1},{},{},{}\n", s.ts, s.percent, s.status.code(), opt(s.power_w), opt(s.health))
}

impl History {
    pub fn load() -> Self {
        let path = gtk::glib::user_data_dir().join("battery-health-center").join("history.csv");
        let cutoff = now_unix() - RETENTION_SECS;
        let samples: Vec<Sample> = fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .filter_map(parse_line)
            .filter(|s| s.ts >= cutoff)
            .collect();
        let history = History { path, samples };
        history.rewrite();
        history
    }

    /// Rewrites the file with only retained samples (prunes old entries).
    fn rewrite(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let data: String = self.samples.iter().map(format_line).collect();
        let _ = fs::write(&self.path, data);
    }

    pub fn last_ts(&self) -> Option<i64> {
        self.samples.last().map(|s| s.ts)
    }

    pub fn record(&mut self, battery: &Battery) {
        let sample = Sample {
            ts: now_unix(),
            percent: battery.percent,
            status: battery.status,
            power_w: battery.power_w,
            health: battery.health_percent(),
        };
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = f.write_all(format_line(&sample).as_bytes());
        }
        self.samples.push(sample);
    }

    /// Average discharge rate in W over the given window, using recorded samples.
    pub fn average_discharge_w(&self, since: i64) -> Option<f64> {
        let vals: Vec<f64> = self
            .samples
            .iter()
            .filter(|s| s.ts >= since && s.status == Status::Discharging)
            .filter_map(|s| s.power_w)
            .filter(|p| *p > 0.0)
            .collect();
        (!vals.is_empty()).then(|| vals.iter().sum::<f64>() / vals.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = Sample { ts: 100, percent: 55.5, status: Status::Charging, power_w: Some(12.3), health: None };
        let back = parse_line(format_line(&s).trim()).unwrap();
        assert_eq!(back.ts, 100);
        assert_eq!(back.status, Status::Charging);
        assert_eq!(back.power_w, Some(12.3));
        assert_eq!(back.health, None);
    }
}
