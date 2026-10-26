//! Battery level history, stored as CSV in ~/.local/share/battery-health-center.
//!
//! Each sample also stores the boot it belongs to and how long the system had
//! been suspended since that boot, so time asleep between two samples can be
//! measured exactly (see `sleep_sessions`).

use std::fs::{self, File, OpenOptions};
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
    /// First 32 bits of the kernel boot ID.
    pub boot: Option<u32>,
    /// Seconds spent suspended since boot (CLOCK_BOOTTIME - CLOCK_MONOTONIC).
    pub suspended: Option<i64>,
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
    /// Held while this process is the one writing the file. In the snap each
    /// launch is a separate process (see main.rs), so a window opened while the
    /// background copy runs must not write too, or every sample is doubled.
    writer_lock: Option<File>,
}

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    gtk::glib::user_data_dir().join("battery-health-center")
}

fn clock_secs(clock: libc::clockid_t) -> Option<i64> {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock_gettime only writes to the timespec we pass.
    (unsafe { libc::clock_gettime(clock, &mut ts) } == 0).then_some(ts.tv_sec)
}

/// Seconds the system has spent suspended since boot.
pub fn suspended_secs() -> Option<i64> {
    Some(clock_secs(libc::CLOCK_BOOTTIME)? - clock_secs(libc::CLOCK_MONOTONIC)?)
}

pub fn boot_id() -> Option<u32> {
    let id = fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    u32::from_str_radix(id.get(..8)?, 16).ok()
}

/// Battery use since the charger was last disconnected.
#[derive(Clone, Debug, PartialEq)]
pub struct SinceCharge {
    /// When the charger was disconnected, or, if the app wasn't running then
    /// (`exact` is false), the last time it saw the laptop plugged in.
    pub unplugged_ts: i64,
    pub exact: bool,
    pub start_percent: f64,
    pub used_percent: f64,
    pub avg_w: Option<f64>,
}

/// A period the laptop spent suspended, with the charge it used.
#[derive(Clone, Debug, PartialEq)]
pub struct SleepSession {
    pub start_ts: i64,
    pub slept_secs: i64,
    pub from_percent: f64,
    pub to_percent: f64,
}

impl SleepSession {
    pub fn percent_per_hour(&self) -> f64 {
        (self.from_percent - self.to_percent).max(0.0) / (self.slept_secs as f64 / 3600.0)
    }
}

/// Samples further apart than this mean the app wasn't running in between.
const UNPLUG_GAP_SECS: i64 = 600;

/// Minimum suspend length worth reporting.
const MIN_SLEEP_SECS: i64 = 15 * 60;

fn parse_line(line: &str) -> Option<Sample> {
    let mut f = line.split(',');
    let ts = f.next()?.parse().ok()?;
    let percent = f.next()?.parse().ok()?;
    let status = Status::from_code(f.next()?.chars().next()?);
    let power_w = f.next().and_then(|s| s.parse().ok());
    let health = f.next().and_then(|s| s.parse().ok());
    let boot = f.next().and_then(|s| u32::from_str_radix(s, 16).ok());
    let suspended = f.next().and_then(|s| s.parse().ok());
    Some(Sample { ts, percent, status, power_w, health, boot, suspended })
}

fn format_line(s: &Sample) -> String {
    let opt = |v: Option<f64>| v.map(|v| format!("{v:.2}")).unwrap_or_default();
    format!(
        "{},{:.1},{},{},{},{},{}\n",
        s.ts,
        s.percent,
        s.status.code(),
        opt(s.power_w),
        opt(s.health),
        s.boot.map(|b| format!("{b:08x}")).unwrap_or_default(),
        s.suspended.map(|v| v.to_string()).unwrap_or_default()
    )
}

impl History {
    pub fn load() -> Self {
        let path = data_dir().join("history.csv");
        let cutoff = now_unix() - RETENTION_SECS;
        let samples: Vec<Sample> = fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .filter_map(parse_line)
            .filter(|s| s.ts >= cutoff)
            .collect();
        let mut history = History { path, samples, writer_lock: None };
        if history.try_become_writer() {
            history.rewrite();
        }
        history
    }

    /// Takes the writer lock if no other process holds it. The OS releases it
    /// when the holder exits, so a later call can take over.
    fn try_become_writer(&mut self) -> bool {
        if self.writer_lock.is_none() {
            if let Some(dir) = self.path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let lock_path = self.path.with_extension("lock");
            if let Ok(file) = OpenOptions::new().create(true).truncate(false).write(true).open(lock_path) {
                if file.try_lock().is_ok() {
                    self.writer_lock = Some(file);
                }
            }
        }
        self.writer_lock.is_some()
    }

    /// Rewrites the file with only retained samples (prunes old entries).
    fn rewrite(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let data: String = self.samples.iter().map(format_line).collect();
        let _ = fs::write(&self.path, data);
    }

    /// Whether this process writes history (and so also sends notifications
    /// and keeps the other logs); see `writer_lock`.
    pub fn is_writer(&self) -> bool {
        self.writer_lock.is_some()
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
            boot: boot_id(),
            suspended: suspended_secs(),
        };
        // Another process records to the file; keep the sample in memory only
        // so this window's charts stay current.
        if self.try_become_writer() {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
                let _ = f.write_all(format_line(&sample).as_bytes());
            }
        }
        self.samples.push(sample);
    }

    /// Battery use since the charger was last disconnected, if on battery now.
    pub fn since_last_charge(&self) -> Option<SinceCharge> {
        since_last_charge(&self.samples)
    }

    /// Suspends of at least 15 minutes, most recent first.
    pub fn sleep_sessions(&self) -> Vec<SleepSession> {
        sleep_sessions(&self.samples)
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

fn since_last_charge(samples: &[Sample]) -> Option<SinceCharge> {
    let last = samples.last().filter(|s| s.status == Status::Discharging)?;
    // Walk back over the current run of on-battery samples.
    let run_start = samples.iter().rposition(|s| s.on_power()).map_or(0, |i| i + 1);
    let run = &samples[run_start..];
    let first = run.first()?;
    // The charge when unplugged is best taken from the last plugged-in sample.
    let before = run_start.checked_sub(1).map(|i| &samples[i]);
    let start_percent = before.map_or(first.percent, |b| b.percent);
    // If samples stop between the last plugged-in one and the first on-battery
    // one, the app wasn't running when the charger came out.
    let gap = before.is_some_and(|b| first.ts - b.ts > UNPLUG_GAP_SECS);
    let watts: Vec<f64> = run.iter().filter_map(|s| s.power_w).filter(|w| *w > 0.0).collect();
    Some(SinceCharge {
        unplugged_ts: if gap { before.map_or(first.ts, |b| b.ts) } else { first.ts },
        exact: !gap,
        start_percent,
        used_percent: (start_percent - last.percent).max(0.0),
        avg_w: (!watts.is_empty()).then(|| watts.iter().sum::<f64>() / watts.len() as f64),
    })
}

fn sleep_sessions(samples: &[Sample]) -> Vec<SleepSession> {
    let mut out: Vec<SleepSession> = samples
        .windows(2)
        .filter_map(|w| {
            let (a, b) = (&w[0], &w[1]);
            if a.boot.is_none() || a.boot != b.boot || a.on_power() || b.on_power() {
                return None;
            }
            let slept = b.suspended? - a.suspended?;
            (slept >= MIN_SLEEP_SECS).then_some(SleepSession {
                start_ts: a.ts,
                slept_secs: slept,
                from_percent: a.percent,
                to_percent: b.percent,
            })
        })
        .collect();
    out.reverse();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = Sample {
            ts: 100,
            percent: 55.5,
            status: Status::Charging,
            power_w: Some(12.3),
            health: None,
            boot: Some(0x7969_1cd8),
            suspended: Some(42),
        };
        let back = parse_line(format_line(&s).trim()).unwrap();
        assert_eq!(back.ts, 100);
        assert_eq!(back.status, Status::Charging);
        assert_eq!(back.power_w, Some(12.3));
        assert_eq!(back.health, None);
        assert_eq!(back.boot, Some(0x7969_1cd8));
        assert_eq!(back.suspended, Some(42));
        // Lines written by older versions have no boot/suspend columns.
        assert_eq!(parse_line("100,50.0,d,10.00,85.00").unwrap().boot, None);
    }

    fn sample(ts: i64, percent: f64, status: Status, suspended: i64) -> Sample {
        Sample { ts, percent, status, power_w: Some(10.0), health: None, boot: Some(1), suspended: Some(suspended) }
    }

    #[test]
    fn since_charge() {
        let samples = vec![
            sample(0, 80.0, Status::NotCharging, 0),
            sample(60, 79.0, Status::Discharging, 0),
            sample(120, 70.0, Status::Discharging, 0),
        ];
        let s = since_last_charge(&samples).unwrap();
        assert_eq!(s.unplugged_ts, 60);
        assert!(s.exact);
        assert_eq!(s.start_percent, 80.0);
        assert_eq!(s.used_percent, 10.0);
        // Not on battery now: nothing to report.
        assert!(since_last_charge(&samples[..1]).is_none());
    }

    #[test]
    fn unplugged_while_the_app_was_closed() {
        // Plugged in at 84%, then nothing for hours; the next sample is on battery.
        let samples = vec![
            sample(0, 84.0, Status::Charging, 0),
            sample(5 * 3600, 20.0, Status::Discharging, 0),
            sample(5 * 3600 + 60, 19.0, Status::Discharging, 0),
        ];
        let s = since_last_charge(&samples).unwrap();
        assert!(!s.exact);
        assert_eq!(s.unplugged_ts, 0, "falls back to when it was last seen plugged in");
        assert_eq!(s.start_percent, 84.0);
        assert_eq!(s.used_percent, 65.0);
    }

    #[test]
    fn sleep() {
        let samples = vec![
            sample(0, 90.0, Status::Discharging, 0),
            // 8 hours asleep, 4% used.
            sample(8 * 3600 + 30, 86.0, Status::Discharging, 8 * 3600),
            // App closed for an hour but no suspend: not a sleep.
            sample(9 * 3600 + 30, 80.0, Status::Discharging, 8 * 3600),
        ];
        let sessions = sleep_sessions(&samples);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].slept_secs, 8 * 3600);
        assert!((sessions[0].percent_per_hour() - 0.5).abs() < 1e-9);
    }
}
