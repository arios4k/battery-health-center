//! Privileged helper for Battery Health Center.
//!
//! Runs as root through pkexec (see data/io.github.BatteryHealthCenter.policy).
//! It only writes the battery charge-control files in sysfs and its own config
//! file, and validates every argument. It intentionally depends on std only.
//!
//! Usage:
//!   bhc-helper set-thresholds <BATTERY> <START|-> <END> [--persist|--no-persist]
//!   bhc-helper set-behaviour <BATTERY> <auto|inhibit-charge|force-discharge>
//!   bhc-helper restore

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const ROOT: &str = "/sys/class/power_supply";
const CONFIG: &str = "/etc/battery-health-center.conf";

fn battery_dir(name: &str) -> Result<PathBuf, String> {
    let valid = !name.is_empty()
        && name.len() <= 32
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !valid {
        return Err(format!("invalid battery name: {name:?}"));
    }
    let dir = PathBuf::from(ROOT).join(name);
    let kind = fs::read_to_string(dir.join("type")).map_err(|_| format!("no such power supply: {name}"))?;
    if kind.trim() != "Battery" {
        return Err(format!("{name} is not a battery"));
    }
    Ok(dir)
}

fn threshold_file(dir: &Path, which: &str) -> Option<PathBuf> {
    let (modern, legacy) = match which {
        "start" => ("charge_control_start_threshold", "charge_start_threshold"),
        _ => ("charge_control_end_threshold", "charge_stop_threshold"),
    };
    [modern, legacy].iter().map(|f| dir.join(f)).find(|p| p.exists())
}

fn read_threshold(dir: &Path, which: &str) -> Option<u8> {
    fs::read_to_string(threshold_file(dir, which)?).ok()?.trim().parse().ok()
}

fn write(path: &PathBuf, value: &str) -> Result<(), String> {
    fs::write(path, value).map_err(|e| format!("writing {}: {e}", path.display()))
}

fn parse_percent(s: &str) -> Result<u8, String> {
    match s.parse::<u8>() {
        Ok(v) if v <= 100 => Ok(v),
        _ => Err(format!("invalid percentage: {s:?}")),
    }
}

fn apply_thresholds(name: &str, start: Option<u8>, end: u8) -> Result<(), String> {
    let dir = battery_dir(name)?;
    if end < 1 {
        return Err("end threshold must be at least 1".into());
    }
    if let Some(s) = start {
        if s >= end {
            return Err("start threshold must be lower than the end threshold".into());
        }
    }
    let end_file = threshold_file(&dir, "end").ok_or("this battery does not support a charge limit")?;
    let start_file = threshold_file(&dir, "start");

    // Firmware usually requires start < end at all times, so the write order
    // depends on whether the window moves up or down.
    let cur_end = read_threshold(&dir, "end").unwrap_or(100);
    match (start, &start_file) {
        (Some(s), Some(sf)) if s >= cur_end => {
            write(&end_file, &end.to_string())?;
            write(sf, &s.to_string())?;
        }
        (Some(s), Some(sf)) => {
            write(sf, &s.to_string())?;
            write(&end_file, &end.to_string())?;
        }
        _ => write(&end_file, &end.to_string())?,
    }
    Ok(())
}

/// Config lines look like: `BAT0 75 80` (start may be `-`).
fn update_config(name: &str, entry: Option<String>) -> Result<(), String> {
    let existing = fs::read_to_string(CONFIG).unwrap_or_default();
    let mut lines: Vec<String> = existing
        .lines()
        .filter(|l| l.split_whitespace().next() != Some(name))
        .map(String::from)
        .collect();
    lines.extend(entry);
    if lines.is_empty() {
        let _ = fs::remove_file(CONFIG);
        return Ok(());
    }
    let mut data = String::from("# Managed by Battery Health Center: <battery> <start|-> <end>\n");
    for l in lines.iter().filter(|l| !l.starts_with('#')) {
        data.push_str(l);
        data.push('\n');
    }
    fs::write(CONFIG, data).map_err(|e| format!("writing {CONFIG}: {e}"))
}

fn restore() -> Result<(), String> {
    let Ok(config) = fs::read_to_string(CONFIG) else {
        return Ok(());
    };
    let mut failed = false;
    for line in config.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let result = match f.as_slice() {
            [name, start, end] => {
                let start = if *start == "-" { Ok(None) } else { parse_percent(start).map(Some) };
                start.and_then(|s| apply_thresholds(name, s, parse_percent(end)?))
            }
            _ => Err(format!("malformed line: {line:?}")),
        };
        if let Err(e) = result {
            eprintln!("bhc-helper: {e}");
            failed = true;
        }
    }
    if failed {
        Err("some limits could not be restored".into())
    } else {
        Ok(())
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["set-thresholds", name, start, end, rest @ ..] => {
            let start = if *start == "-" { None } else { Some(parse_percent(start)?) };
            let end = parse_percent(end)?;
            let persist = match rest {
                [] => None,
                ["--persist"] => Some(true),
                ["--no-persist"] => Some(false),
                _ => return Err("unknown option".into()),
            };
            apply_thresholds(name, start, end)?;
            match persist {
                Some(true) => {
                    let s = start.map(|s| s.to_string()).unwrap_or_else(|| "-".into());
                    update_config(name, Some(format!("{name} {s} {end}")))
                }
                Some(false) => update_config(name, None),
                None => Ok(()),
            }
        }
        ["set-behaviour", name, mode] => {
            if !["auto", "inhibit-charge", "force-discharge"].contains(mode) {
                return Err(format!("invalid charge behaviour: {mode:?}"));
            }
            let dir = battery_dir(name)?;
            write(&dir.join("charge_behaviour"), mode)
        }
        ["restore"] => restore(),
        _ => Err("usage: bhc-helper set-thresholds <BAT> <START|-> <END> [--persist|--no-persist]\n       \
                  bhc-helper set-behaviour <BAT> <auto|inhibit-charge|force-discharge>\n       \
                  bhc-helper restore"
            .into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
