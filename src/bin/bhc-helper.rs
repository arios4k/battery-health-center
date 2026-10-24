//! Privileged helper for Battery Health Center.
//!
//! Runs as root, either through pkexec (see
//! data/io.github.BatteryHealthCenter.policy) or, in the snap, as a service
//! the app talks to over a Unix socket (pkexec can't be used inside a snap).
//! It only writes the battery charge-control files in sysfs and its own config
//! file, and validates every argument. It intentionally depends on std only.
//!
//! Usage:
//!   bhc-helper set-thresholds <BATTERY> <START|-> <END> [--persist|--no-persist]
//!   bhc-helper set-behaviour <BATTERY> <auto|inhibit-charge|force-discharge>
//!   bhc-helper restore
//!   bhc-helper serve        (snap service; accepts set-thresholds requests)

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

const ROOT: &str = "/sys/class/power_supply";
pub const SOCKET_NAME: &str = "helper.sock";

/// $SNAP_COMMON when running inside the snap.
fn snap_common() -> Option<PathBuf> {
    std::env::var_os("SNAP_COMMON").map(PathBuf::from)
}

fn config_path() -> PathBuf {
    match snap_common() {
        Some(dir) => dir.join("battery-health-center.conf"),
        None => PathBuf::from("/etc/battery-health-center.conf"),
    }
}

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
    // Inside the snap, the power-control interface only allows writing the
    // legacy names, so prefer those there.
    let order = if snap_common().is_some() { [legacy, modern] } else { [modern, legacy] };
    order.iter().map(|f| dir.join(f)).find(|p| p.exists())
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
    let cur_start = read_threshold(&dir, "start");
    if cur_end == end && (start.is_none() || start == cur_start) {
        return Ok(());
    }
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
    let config = config_path();
    let existing = fs::read_to_string(&config).unwrap_or_default();
    let mut lines: Vec<String> = existing
        .lines()
        .filter(|l| l.split_whitespace().next() != Some(name))
        .map(String::from)
        .collect();
    lines.extend(entry);
    if lines.is_empty() {
        let _ = fs::remove_file(&config);
        return Ok(());
    }
    let mut data = String::from("# Managed by Battery Health Center: <battery> <start|-> <end>\n");
    for l in lines.iter().filter(|l| !l.starts_with('#')) {
        data.push_str(l);
        data.push('\n');
    }
    fs::write(&config, data).map_err(|e| format!("writing {}: {e}", config.display()))
}

fn restore() -> Result<(), String> {
    let Ok(config) = fs::read_to_string(config_path()) else {
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
        ["serve"] => serve(),
        _ => Err("usage: bhc-helper set-thresholds <BAT> <START|-> <END> [--persist|--no-persist]\n       \
                  bhc-helper set-behaviour <BAT> <auto|inhibit-charge|force-discharge>\n       \
                  bhc-helper restore"
            .into()),
    }
}

/// Snap service: restores saved limits, re-applies them if firmware resets
/// them (e.g. after sleep), and accepts `set-thresholds` requests from the
/// app over a Unix socket in $SNAP_COMMON. One request per connection; the
/// reply is `OK` or `ERR <message>`.
fn serve() -> Result<(), String> {
    let dir = snap_common().ok_or("serve is only used inside the snap")?;
    let path = dir.join(SOCKET_NAME);
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|e| format!("binding {}: {e}", path.display()))?;
    // Any local user may change the charge limit, as with GNOME's own setting.
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).map_err(|e| e.to_string())?;

    std::thread::spawn(|| loop {
        if let Err(e) = restore() {
            eprintln!("bhc-helper: {e}");
        }
        std::thread::sleep(Duration::from_secs(60));
    });

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut line = String::new();
        if BufReader::new((&stream).take(512)).read_line(&mut line).is_err() {
            continue;
        }
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        let reply = match args.first().map(String::as_str) {
            Some("set-thresholds") => match run(&args) {
                Ok(()) => "OK\n".to_string(),
                Err(e) => format!("ERR {}\n", e.replace('\n', " ")),
            },
            _ => "ERR unsupported request\n".to_string(),
        };
        let _ = stream.write_all(reply.as_bytes());
    }
    Ok(())
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
