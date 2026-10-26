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
//!   bhc-helper charge-full <BATTERY>          (charge to 100% once)
//!   bhc-helper finish-full-charge <BATTERY>   (put the previous limit back)
//!   bhc-helper restore
//!   bhc-helper serve        (snap service; accepts set-thresholds,
//!                            charge-full and finish-full-charge requests)

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::time::Duration;

/// Serializes sysfs writes between the service's request handler and its
/// background loop.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
use tests::{state_dir, sysfs_root};

#[cfg(not(test))]
fn sysfs_root() -> PathBuf {
    PathBuf::from("/sys/class/power_supply")
}
pub const SOCKET_NAME: &str = "helper.sock";
/// Present while snapd is holding back an update of this snap because the app
/// is running; the app's background copy exits when it sees it.
pub const REFRESH_PENDING_NAME: &str = "refresh-pending";
/// How often the service asks snapd about pending updates (in 30 s rounds).
const REFRESH_CHECK_ROUNDS: u32 = 10;

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

/// Where "charge to 100%" markers live: $SNAP_COMMON in the snap (readable by
/// the app), otherwise /run (cleared at boot, when the saved limit applies).
#[cfg(not(test))]
fn state_dir() -> PathBuf {
    snap_common().unwrap_or_else(|| PathBuf::from("/run/battery-health-center"))
}

/// Marker for a one-time full charge; holds the limit to put back
/// ("<start|-> <end>").
fn full_charge_marker(name: &str) -> PathBuf {
    state_dir().join(format!("full-charge-{name}"))
}

fn battery_dir(name: &str) -> Result<PathBuf, String> {
    let valid = !name.is_empty()
        && name.len() <= 32
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !valid {
        return Err(format!("invalid battery name: {name:?}"));
    }
    let dir = sysfs_root().join(name);
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
            [name, _, _] if full_charge_marker(name).exists() => Ok(()),
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

/// Raises the limit so the battery charges to 100% once, remembering the
/// current limit so `finish_full_charge` can put it back.
fn charge_full(name: &str) -> Result<(), String> {
    let dir = battery_dir(name)?;
    let marker = full_charge_marker(name);
    if marker.exists() {
        return Ok(());
    }
    let end = read_threshold(&dir, "end").ok_or("this battery does not support a charge limit")?;
    let start = read_threshold(&dir, "start");
    let _ = fs::create_dir_all(state_dir());
    let saved = format!("{} {end}\n", start.map(|s| s.to_string()).unwrap_or_else(|| "-".into()));
    fs::write(&marker, saved).map_err(|e| format!("writing {}: {e}", marker.display()))?;
    // With a start threshold, charging only begins below it, so raise it too.
    let result = apply_thresholds(name, start.map(|_| 99), 100);
    if result.is_err() {
        let _ = fs::remove_file(&marker);
    }
    result
}

/// Puts back the limit saved by `charge_full`.
fn finish_full_charge(name: &str) -> Result<(), String> {
    battery_dir(name)?;
    let marker = full_charge_marker(name);
    let Ok(saved) = fs::read_to_string(&marker) else {
        return Ok(());
    };
    let f: Vec<&str> = saved.split_whitespace().collect();
    let [start, end] = f.as_slice() else {
        let _ = fs::remove_file(&marker);
        return Err("the saved limit was unreadable; set the limit again".into());
    };
    let start = if *start == "-" { None } else { Some(parse_percent(start)?) };
    apply_thresholds(name, start, parse_percent(end)?)?;
    fs::remove_file(&marker).map_err(|e| e.to_string())
}

/// Finishes one-time full charges once the battery is full or unplugged.
fn check_full_charges() {
    let Ok(entries) = fs::read_dir(state_dir()) else { return };
    let ac_online = fs::read_dir(sysfs_root()).into_iter().flatten().flatten().any(|e| {
        let p = e.path();
        fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Mains")
            && fs::read_to_string(p.join("online")).is_ok_and(|o| o.trim() == "1")
    });
    for name in entries.flatten().filter_map(|e| e.file_name().to_str()?.strip_prefix("full-charge-").map(String::from)) {
        let Ok(dir) = battery_dir(&name) else { continue };
        let read = |f: &str| fs::read_to_string(dir.join(f)).unwrap_or_default().trim().to_string();
        let full = read("status") == "Full" || read("capacity").parse::<u8>().is_ok_and(|c| c >= 99);
        if full || !ac_online {
            if let Err(e) = finish_full_charge(&name) {
                eprintln!("bhc-helper: {e}");
            }
        }
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
        ["charge-full", name] => charge_full(name),
        ["finish-full-charge", name] => finish_full_charge(name),
        ["restore"] => restore(),
        ["serve"] => serve(),
        _ => Err("usage: bhc-helper set-thresholds <BAT> <START|-> <END> [--persist|--no-persist]\n       \
                  bhc-helper set-behaviour <BAT> <auto|inhibit-charge|force-discharge>\n       \
                  bhc-helper charge-full <BAT>\n       \
                  bhc-helper finish-full-charge <BAT>\n       \
                  bhc-helper restore"
            .into()),
    }
}

fn run_locked(args: &[String]) -> Result<(), String> {
    let _guard = WRITE_LOCK.lock();
    run(args)
}

/// Parses `snapctl refresh --pending` output ("pending: inhibited", ...).
fn refresh_inhibited(output: &str) -> bool {
    output.lines().any(|l| l.split_once(':').is_some_and(|(k, v)| k.trim() == "pending" && v.trim() == "inhibited"))
}

/// Only root may ask snapd whether an update is waiting for the app to
/// close, so the service asks and leaves a marker the app can see.
fn update_refresh_marker(marker: &Path) {
    let Ok(out) = std::process::Command::new("snapctl").args(["refresh", "--pending"]).output() else {
        return;
    };
    if refresh_inhibited(&String::from_utf8_lossy(&out.stdout)) {
        let _ = fs::write(marker, "");
        let _ = fs::set_permissions(marker, fs::Permissions::from_mode(0o644));
    } else {
        let _ = fs::remove_file(marker);
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

    // This instance runs the new revision, so any marker is stale.
    let marker = dir.join(REFRESH_PENDING_NAME);
    let _ = fs::remove_file(&marker);

    std::thread::spawn(move || {
        let mut round: u32 = 0;
        loop {
            {
                let _guard = WRITE_LOCK.lock();
                check_full_charges();
                if let Err(e) = restore() {
                    eprintln!("bhc-helper: {e}");
                }
            }
            if round.is_multiple_of(REFRESH_CHECK_ROUNDS) {
                update_refresh_marker(&marker);
            }
            round = round.wrapping_add(1);
            std::thread::sleep(Duration::from_secs(30));
        }
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
            Some("set-thresholds" | "charge-full" | "finish-full-charge") => match run_locked(&args) {
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

/// Tests run against a fake power_supply folder in a temp directory.
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static DIR: RefCell<PathBuf> = const { RefCell::new(PathBuf::new()) };
    }

    pub fn sysfs_root() -> PathBuf {
        DIR.with(|d| d.borrow().join("sys"))
    }

    pub fn state_dir() -> PathBuf {
        DIR.with(|d| d.borrow().join("state"))
    }

    fn set(file: &str, value: &str) {
        fs::write(sysfs_root().join(file), value).unwrap();
    }

    fn get(file: &str) -> String {
        fs::read_to_string(sysfs_root().join(file)).unwrap().trim().to_string()
    }

    /// A battery limited to 75–80%, at 80% and plugged in.
    fn setup(name: &str) {
        let dir = std::env::temp_dir().join(format!("bhc-helper-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        DIR.with(|d| *d.borrow_mut() = dir);
        fs::create_dir_all(sysfs_root().join("BAT0")).unwrap();
        fs::create_dir_all(sysfs_root().join("AC")).unwrap();
        set("BAT0/type", "Battery\n");
        set("BAT0/charge_control_start_threshold", "75\n");
        set("BAT0/charge_control_end_threshold", "80\n");
        set("BAT0/capacity", "80\n");
        set("BAT0/status", "Not charging\n");
        set("AC/type", "Mains\n");
        set("AC/online", "1\n");
    }

    fn limits() -> (String, String) {
        (get("BAT0/charge_control_start_threshold"), get("BAT0/charge_control_end_threshold"))
    }

    #[test]
    fn full_charge_raises_and_restores() {
        setup("manual");
        charge_full("BAT0").unwrap();
        assert_eq!(limits(), ("99".into(), "100".into()));
        assert_eq!(fs::read_to_string(full_charge_marker("BAT0")).unwrap().trim(), "75 80");
        // A second request doesn't overwrite the saved limit.
        charge_full("BAT0").unwrap();
        assert_eq!(fs::read_to_string(full_charge_marker("BAT0")).unwrap().trim(), "75 80");
        finish_full_charge("BAT0").unwrap();
        assert_eq!(limits(), ("75".into(), "80".into()));
        assert!(!full_charge_marker("BAT0").exists());
    }

    #[test]
    fn finishes_when_full() {
        setup("full");
        charge_full("BAT0").unwrap();
        set("BAT0/capacity", "95\n");
        check_full_charges();
        assert_eq!(limits(), ("99".into(), "100".into()), "still charging at 95%");
        set("BAT0/capacity", "100\n");
        check_full_charges();
        assert_eq!(limits(), ("75".into(), "80".into()));
    }

    #[test]
    fn finishes_when_unplugged() {
        setup("unplugged");
        charge_full("BAT0").unwrap();
        set("AC/online", "0\n");
        check_full_charges();
        assert_eq!(limits(), ("75".into(), "80".into()));
        assert!(!full_charge_marker("BAT0").exists());
    }

    #[test]
    fn parses_pending_refresh() {
        assert!(refresh_inhibited("pending: inhibited\nchannel: stable\nversion: 0.2.1\n"));
        assert!(!refresh_inhibited("pending: ready\nchannel: stable\n"));
        assert!(!refresh_inhibited("pending: none\n"));
        assert!(!refresh_inhibited(""));
    }

    #[test]
    fn rejects_bad_names() {
        setup("names");
        assert!(charge_full("../BAT0").is_err());
        assert!(charge_full("AC").is_err());
    }
}
