//! Reads battery information from the kernel's power_supply class in sysfs.
//!
//! Units in sysfs: energy in µWh, charge in µAh, power in µW, current in µA,
//! voltage in µV. Everything is converted to Wh / W / V here.

use std::fs;
use std::path::{Path, PathBuf};

pub const POWER_SUPPLY_ROOT: &str = "/sys/class/power_supply";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Charging,
    Discharging,
    NotCharging,
    Full,
    Unknown,
}

impl Status {
    fn parse(s: &str) -> Self {
        match s {
            "Charging" => Status::Charging,
            "Discharging" => Status::Discharging,
            "Not charging" => Status::NotCharging,
            "Full" => Status::Full,
            _ => Status::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Charging => "Charging",
            Status::Discharging => "On Battery",
            Status::NotCharging => "Plugged In, Not Charging",
            Status::Full => "Fully Charged",
            Status::Unknown => "Unknown",
        }
    }

    /// A short form that fits inside the charge ring; the full phrase is shown
    /// beside it.
    pub fn short_label(self) -> &'static str {
        match self {
            Status::Charging => "Charging",
            Status::Discharging => "On Battery",
            Status::NotCharging => "Not Charging",
            Status::Full => "Full",
            Status::Unknown => "",
        }
    }

    pub fn code(self) -> char {
        match self {
            Status::Charging => 'c',
            Status::Discharging => 'd',
            Status::NotCharging => 'n',
            Status::Full => 'f',
            Status::Unknown => 'u',
        }
    }

    pub fn from_code(c: char) -> Self {
        match c {
            'c' => Status::Charging,
            'd' => Status::Discharging,
            'n' => Status::NotCharging,
            'f' => Status::Full,
            _ => Status::Unknown,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Battery {
    /// sysfs name, e.g. "BAT0"
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub technology: Option<String>,
    pub status: Status,
    pub percent: f64,
    pub energy_now_wh: Option<f64>,
    pub energy_full_wh: Option<f64>,
    pub energy_design_wh: Option<f64>,
    pub power_w: Option<f64>,
    pub voltage_v: Option<f64>,
    pub voltage_design_v: Option<f64>,
    pub cycle_count: Option<u32>,
    pub start_threshold: Option<u8>,
    pub end_threshold: Option<u8>,
    /// Available charge behaviours (e.g. auto, inhibit-charge, force-discharge)
    pub charge_behaviours: Vec<String>,
    pub charge_behaviour: Option<String>,
    pub ac_online: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Normal,
    Worn,
    ServiceRecommended,
}

impl Condition {
    pub fn label(self) -> &'static str {
        match self {
            Condition::Normal => "Normal",
            Condition::Worn => "Worn",
            Condition::ServiceRecommended => "Service Recommended",
        }
    }
}

impl Battery {
    /// Maximum capacity relative to when new (full charge / design capacity).
    pub fn health_percent(&self) -> Option<f64> {
        match (self.energy_full_wh, self.energy_design_wh) {
            (Some(full), Some(design)) if design > 0.0 => Some((full / design * 100.0).min(100.0)),
            _ => None,
        }
    }

    pub fn condition(&self) -> Option<Condition> {
        let h = self.health_percent()?;
        Some(if h >= 80.0 {
            Condition::Normal
        } else if h >= 60.0 {
            Condition::Worn
        } else {
            Condition::ServiceRecommended
        })
    }

    pub fn supports_end_threshold(&self) -> bool {
        self.end_threshold.is_some()
    }

    pub fn supports_start_threshold(&self) -> bool {
        self.start_threshold.is_some()
    }

    pub fn is_on_battery(&self) -> bool {
        self.status == Status::Discharging
    }

    /// Hours until empty (discharging) or until the charge limit (charging).
    pub fn hours_remaining(&self) -> Option<f64> {
        let power = self.power_w.filter(|p| *p > 0.1)?;
        let now = self.energy_now_wh?;
        match self.status {
            Status::Discharging => Some(now / power),
            Status::Charging => {
                let full = self.energy_full_wh?;
                let limit = self.end_threshold.unwrap_or(100).min(100) as f64 / 100.0;
                let target = full * limit;
                (target > now).then(|| (target - now) / power)
            }
            _ => None,
        }
    }
}

fn read_str(dir: &Path, file: &str) -> Option<String> {
    let s = fs::read_to_string(dir.join(file)).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn read_num(dir: &Path, file: &str) -> Option<f64> {
    read_str(dir, file)?.parse::<f64>().ok()
}

/// All system batteries (excludes peripheral batteries such as mice).
pub fn battery_paths() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(POWER_SUPPLY_ROOT) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| read_str(p, "type").as_deref() == Some("Battery"))
        .filter(|p| read_str(p, "scope").as_deref() != Some("Device"))
        .filter(|p| read_str(p, "present").as_deref() != Some("0"))
        .collect();
    paths.sort();
    paths
}

fn ac_online() -> bool {
    let Ok(entries) = fs::read_dir(POWER_SUPPLY_ROOT) else {
        return false;
    };
    entries.flatten().map(|e| e.path()).any(|p| {
        read_str(&p, "type").as_deref() == Some("Mains") && read_str(&p, "online").as_deref() == Some("1")
    })
}

/// Parses "[auto] inhibit-charge force-discharge" into (all, selected).
fn parse_behaviours(s: &str) -> (Vec<String>, Option<String>) {
    let mut all = Vec::new();
    let mut selected = None;
    for word in s.split_whitespace() {
        if let Some(w) = word.strip_prefix('[').and_then(|w| w.strip_suffix(']')) {
            selected = Some(w.to_string());
            all.push(w.to_string());
        } else {
            all.push(word.to_string());
        }
    }
    (all, selected)
}

pub fn read_battery(dir: &Path) -> Option<Battery> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    let voltage_v = read_num(dir, "voltage_now").map(|v| v / 1e6);
    let voltage_design_v = read_num(dir, "voltage_min_design").map(|v| v / 1e6);

    // Prefer energy_* (µWh). Some drivers only report charge_* (µAh), which we
    // convert using the design voltage.
    let (energy_now_wh, energy_full_wh, energy_design_wh) = if read_str(dir, "energy_full").is_some() {
        (
            read_num(dir, "energy_now").map(|v| v / 1e6),
            read_num(dir, "energy_full").map(|v| v / 1e6),
            read_num(dir, "energy_full_design").map(|v| v / 1e6),
        )
    } else {
        let volts = voltage_design_v.or(voltage_v).unwrap_or(0.0);
        let conv = |v: f64| v / 1e6 * volts;
        (
            read_num(dir, "charge_now").map(conv),
            read_num(dir, "charge_full").map(conv),
            read_num(dir, "charge_full_design").map(conv),
        )
    };

    let power_w = read_num(dir, "power_now").map(|p| p.abs() / 1e6).or_else(|| {
        let current = read_num(dir, "current_now")?;
        Some((current * voltage_v? * 1e6).abs() / 1e12)
    });

    let percent = read_num(dir, "capacity").unwrap_or_else(|| match (energy_now_wh, energy_full_wh) {
        (Some(n), Some(f)) if f > 0.0 => n / f * 100.0,
        _ => 0.0,
    });

    let threshold = |a: &str, b: &str| {
        read_num(dir, a).or_else(|| read_num(dir, b)).map(|v| v.clamp(0.0, 100.0) as u8)
    };
    let (charge_behaviours, charge_behaviour) =
        read_str(dir, "charge_behaviour").map(|s| parse_behaviours(&s)).unwrap_or_default();

    let status = read_str(dir, "status").map(|s| Status::parse(&s)).unwrap_or(Status::Unknown);
    Some(Battery {
        name,
        manufacturer: read_str(dir, "manufacturer"),
        model: read_str(dir, "model_name"),
        serial: read_str(dir, "serial_number"),
        technology: read_str(dir, "technology"),
        status,
        percent: percent.clamp(0.0, 100.0),
        energy_now_wh,
        energy_full_wh,
        energy_design_wh,
        power_w,
        voltage_v,
        voltage_design_v,
        cycle_count: read_num(dir, "cycle_count").map(|c| c as u32).filter(|c| *c > 0),
        start_threshold: threshold("charge_control_start_threshold", "charge_start_threshold"),
        end_threshold: threshold("charge_control_end_threshold", "charge_stop_threshold"),
        charge_behaviours,
        charge_behaviour,
        ac_online: ac_online() || matches!(status, Status::Charging | Status::Full | Status::NotCharging),
    })
}

/// All system batteries, in name order.
pub fn all_batteries() -> Vec<Battery> {
    battery_paths().iter().filter_map(|p| read_battery(p)).collect()
}

/// Combines several batteries into one overall view: energy-weighted charge,
/// summed capacity and power. Identity fields and thresholds come from the
/// first battery. With one battery it is returned as is.
pub fn combine(batteries: &[Battery]) -> Option<Battery> {
    let first = batteries.first()?;
    if batteries.len() == 1 {
        return Some(first.clone());
    }
    let sum = |f: fn(&Battery) -> Option<f64>| -> Option<f64> {
        batteries.iter().map(f).sum::<Option<f64>>()
    };
    let mut b = first.clone();
    b.name = batteries.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(" + ");
    b.energy_now_wh = sum(|b| b.energy_now_wh);
    b.energy_full_wh = sum(|b| b.energy_full_wh);
    b.energy_design_wh = sum(|b| b.energy_design_wh);
    b.power_w = batteries.iter().filter_map(|b| b.power_w).reduce(|a, p| a + p);
    if let (Some(now), Some(full)) = (b.energy_now_wh, b.energy_full_wh) {
        if full > 0.0 {
            b.percent = (now / full * 100.0).clamp(0.0, 100.0);
        }
    }
    let any = |st: Status| batteries.iter().any(|b| b.status == st);
    b.status = if any(Status::Charging) {
        Status::Charging
    } else if any(Status::Discharging) {
        Status::Discharging
    } else {
        first.status
    };
    b.ac_online = batteries.iter().any(|b| b.ac_online);
    Some(b)
}

/// The connected charger, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Charger {
    /// e.g. "USB-C Power Delivery" or "AC adapter"
    pub kind: String,
    pub volts: Option<f64>,
    pub amps: Option<f64>,
}

impl Charger {
    pub fn watts(&self) -> Option<f64> {
        Some(self.volts? * self.amps?).filter(|w| *w > 0.0)
    }
}

/// Reads the charger from USB-C sources (which report the negotiated
/// voltage and current) or a plain AC adapter (which reports neither).
pub fn charger() -> Option<Charger> {
    let entries = fs::read_dir(POWER_SUPPLY_ROOT).ok()?;
    let mut mains = false;
    let mut best: Option<Charger> = None;
    for p in entries.flatten().map(|e| e.path()) {
        if read_str(&p, "online").as_deref() != Some("1") {
            continue;
        }
        match read_str(&p, "type").as_deref() {
            Some("Mains") => mains = true,
            Some(t) if t.starts_with("USB") => {
                let usb_type = read_str(&p, "usb_type").unwrap_or_default();
                // The active mode is in brackets, e.g. "C [PD] PD_PPS".
                let active = usb_type.split_whitespace().find(|w| w.starts_with('[')).unwrap_or("");
                let kind = if active.contains("PD") { "USB-C Power Delivery" } else { "USB-C" };
                let volts = read_num(&p, "voltage_max").or_else(|| read_num(&p, "voltage_now")).map(|v| v / 1e6).filter(|v| *v > 0.0);
                let amps = read_num(&p, "current_max").map(|a| a / 1e6).filter(|a| *a > 0.0);
                let c = Charger { kind: kind.into(), volts, amps };
                if best.as_ref().is_none_or(|b| c.watts().unwrap_or(0.0) > b.watts().unwrap_or(0.0)) {
                    best = Some(c);
                }
            }
            _ => {}
        }
    }
    best.or_else(|| mains.then(|| Charger { kind: "AC adapter".into(), volts: None, amps: None }))
}

/// The selected and available system sleep modes from /sys/power/mem_sleep,
/// e.g. ("s2idle", ["s2idle", "deep"]).
pub fn sleep_mode() -> Option<(String, Vec<String>)> {
    let text = fs::read_to_string("/sys/power/mem_sleep").ok()?;
    let mut selected = None;
    let modes: Vec<String> = text
        .split_whitespace()
        .map(|w| match w.strip_prefix('[').and_then(|w| w.strip_suffix(']')) {
            Some(m) => {
                selected = Some(m.to_string());
                m.to_string()
            }
            None => w.to_string(),
        })
        .collect();
    Some((selected?, modes))
}

pub fn format_hours(hours: f64) -> String {
    let total_min = (hours * 60.0).round() as i64;
    let (h, m) = (total_min / 60, total_min % 60);
    match (h, m) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaviours() {
        let (all, sel) = parse_behaviours("[auto] inhibit-charge force-discharge");
        assert_eq!(all, vec!["auto", "inhibit-charge", "force-discharge"]);
        assert_eq!(sel.as_deref(), Some("auto"));
    }

    #[test]
    fn hours() {
        assert_eq!(format_hours(0.5), "30 min");
        assert_eq!(format_hours(2.0), "2 h");
        assert_eq!(format_hours(1.25), "1 h 15 min");
    }
}
