//! Privileged helper for Battery Health Center.
//!
//! Runs as root, either through pkexec (see
//! data/io.github.BatteryHealthCenter.policy) or, in the snap, as a service
//! the app talks to over a Unix socket (pkexec can't be used inside a snap).
//! It only writes the battery charge-control files in sysfs and its own config
//! file, and validates every argument. It intentionally depends on little: std,
//! and libc for the local time of day.
//!
//! Usage:
//!   bhc-helper set-thresholds <BATTERY> <START|-> <END> [--persist|--no-persist]
//!   bhc-helper set-behaviour <BATTERY> <auto|inhibit-charge|force-discharge>
//!   bhc-helper charge-full <BATTERY>          (charge to 100% once)
//!   bhc-helper finish-full-charge <BATTERY>   (put the previous limit back)
//!   bhc-helper set-schedule <0|1> <LEAD-MINUTES> [<DAYS>@<HH:MM>]...   (see schedule.rs)
//!   bhc-helper schedule-daemon   (runs the schedule while a schedule is set)
//!   bhc-helper restore
//!   bhc-helper serve        (snap service; accepts set-thresholds,
//!                            charge-full, finish-full-charge and set-schedule
//!                            requests, and runs the schedule)

#[allow(dead_code)]
#[path = "../schedule.rs"]
mod schedule;

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
use tests::{config_path, data_dir, schedule_path, state_dir, sysfs_root};

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

#[cfg(not(test))]
fn config_path() -> PathBuf {
    match snap_common() {
        Some(dir) => dir.join("battery-health-center.conf"),
        None => PathBuf::from("/etc/battery-health-center.conf"),
    }
}

/// The charging schedule (see schedule.rs); readable by the app.
#[cfg(not(test))]
fn schedule_path() -> PathBuf {
    match snap_common() {
        Some(dir) => dir.join("battery-health-center.schedule"),
        None => PathBuf::from("/etc/battery-health-center.schedule"),
    }
}

/// What optimized charging learns from (when you unplug); it has to survive
/// reboots, unlike the markers in `state_dir`.
#[cfg(not(test))]
fn data_dir() -> PathBuf {
    snap_common().unwrap_or_else(|| PathBuf::from("/var/lib/battery-health-center"))
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

/// Marker for a charge-to-100% window from the schedule; the saved limit stays
/// in the config file.
fn scheduled_marker(name: &str) -> PathBuf {
    state_dir().join(format!("scheduled-full-{name}"))
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
            [name, _, _] if full_charge_marker(name).exists() || scheduled_marker(name).exists() => Ok(()),
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
    // The limit to put back is the saved one, not the lifted one.
    end_scheduled_full(name)?;
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

fn ac_online() -> bool {
    fs::read_dir(sysfs_root()).into_iter().flatten().flatten().any(|e| {
        let p = e.path();
        fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Mains")
            && fs::read_to_string(p.join("online")).is_ok_and(|o| o.trim() == "1")
    })
}

/// The saved limits: (battery, start, end) for each line of the config file.
fn saved_limits() -> Vec<(String, Option<u8>, u8)> {
    let config = fs::read_to_string(config_path()).unwrap_or_default();
    config
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let [name, start, end] = f.as_slice() else { return None };
            let start = if *start == "-" { None } else { Some(parse_percent(start).ok()?) };
            Some((name.to_string(), start, parse_percent(end).ok()?))
        })
        .collect()
}

/// Ends a charge-to-100% window from the schedule, putting the saved limit back.
fn end_scheduled_full(name: &str) -> Result<(), String> {
    let marker = scheduled_marker(name);
    if !marker.exists() {
        return Ok(());
    }
    if let Some((_, start, end)) = saved_limits().into_iter().find(|(n, _, _)| n == name) {
        apply_thresholds(name, start, end)?;
    }
    let _ = fs::remove_file(&marker);
    Ok(())
}

/// Finishes one-time full charges once the battery is full or unplugged.
fn check_full_charges() {
    let Ok(entries) = fs::read_dir(state_dir()) else { return };
    let ac_online = ac_online();
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

/// Unplug times kept for learning.
const MAX_UNPLUG_LOG: usize = 60;

fn unplug_log_path() -> PathBuf {
    data_dir().join("unplug-log")
}

fn read_unplug_log() -> Vec<i64> {
    fs::read_to_string(unplug_log_path()).unwrap_or_default().lines().filter_map(|l| l.trim().parse().ok()).collect()
}

fn write_public(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    }
    fs::write(path, text).map_err(|e| format!("writing {}: {e}", path.display()))?;
    // The app reads these to show what was learned.
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o644));
    Ok(())
}

/// While optimized charging is on, notes when the charger comes out after a
/// long enough session. `first_tick` is the first look since this process
/// started: an unplug seen then may have happened any time before, so it
/// isn't recorded.
fn track_plug(optimized: bool, now: i64, first_tick: bool) {
    let since_path = data_dir().join("plugged-since");
    if !optimized {
        let _ = fs::remove_file(&since_path);
        return;
    }
    let since: Option<i64> = fs::read_to_string(&since_path).ok().and_then(|t| t.trim().parse().ok());
    match (ac_online(), since) {
        (true, None) => {
            let _ = write_public(&since_path, &format!("{now}\n"));
        }
        (false, Some(t)) => {
            if !first_tick && now - t >= schedule::MIN_SESSION_SECS {
                let mut log = read_unplug_log();
                log.push(now);
                let keep = log.len().saturating_sub(MAX_UNPLUG_LOG);
                let text: String = log[keep..].iter().map(|ts| format!("{ts}\n")).collect();
                let _ = write_public(&unplug_log_path(), &text);
            }
            let _ = fs::remove_file(&since_path);
        }
        _ => {}
    }
}

/// One look at the schedule: lifts the saved limit to 100% while a ready-by
/// time is near, and puts it back afterwards (or when the schedule is removed).
fn schedule_tick_at(now: i64, clock: schedule::Clock, first_tick: bool) -> Result<(), String> {
    let plan = fs::read_to_string(schedule_path()).map(|t| schedule::Schedule::parse(&t)).unwrap_or_default();
    track_plug(plan.optimized, now, first_tick);
    let learned = if plan.optimized { schedule::learn(&read_unplug_log(), now) } else { schedule::Learned::default() };
    let want = !plan.is_empty() && schedule::full_charge_wanted(&plan, &learned, clock);
    let mut failed = None;
    for (name, start, end) in saved_limits() {
        let result = if want && end < 100 {
            if full_charge_marker(&name).exists() {
                continue; // a one-time full charge is already under way
            }
            let _ = fs::create_dir_all(state_dir());
            fs::write(scheduled_marker(&name), "scheduled\n")
                .map_err(|e| e.to_string())
                .and_then(|()| apply_thresholds(&name, start.map(|_| 99), 100))
        } else {
            end_scheduled_full(&name)
        };
        if let Err(e) = result {
            failed = Some(e);
        }
    }
    failed.map_or(Ok(()), Err)
}

fn schedule_tick(first_tick: bool) -> Result<(), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let clock = schedule::clock_at(now).ok_or("could not read the local time")?;
    schedule_tick_at(now, clock, first_tick)
}

/// Deb service: follows the schedule for as long as there is one.
fn schedule_daemon() -> Result<(), String> {
    let mut first = true;
    loop {
        if let Err(e) = schedule_tick(first) {
            eprintln!("bhc-helper: {e}");
        }
        first = false;
        // Once the schedule is gone the tick above has put the limit back.
        if !schedule_path().exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(30));
    }
}

/// `set-schedule <0|1> <LEAD-MINUTES> [<DAYS>@<HH:MM>]...` writes the schedule
/// and carries it out right away.
fn set_schedule(optimized: &str, lead: &str, rules: &[&str]) -> Result<(), String> {
    let optimized = match optimized {
        "0" => false,
        "1" => true,
        _ => return Err("optimized must be 0 or 1".into()),
    };
    let lead_min: u16 = lead.parse().map_err(|_| format!("invalid lead time: {lead:?}"))?;
    if !(schedule::MIN_LEAD_MIN..=schedule::MAX_LEAD_MIN).contains(&lead_min) {
        return Err(format!("the lead time must be {}-{} minutes", schedule::MIN_LEAD_MIN, schedule::MAX_LEAD_MIN));
    }
    if rules.len() > schedule::MAX_RULES {
        return Err(format!("at most {} times", schedule::MAX_RULES));
    }
    let mut plan = schedule::Schedule { optimized, lead_min, rules: Vec::new() };
    for rule in rules {
        let (days, time) = rule.split_once('@').ok_or_else(|| format!("invalid time: {rule:?}"))?;
        let days: u8 = days.parse().map_err(|_| format!("invalid days: {days:?}"))?;
        if days == 0 || days > schedule::EVERY_DAY {
            return Err(format!("invalid days: {days}"));
        }
        let ready = schedule::parse_time(time).ok_or_else(|| format!("invalid time: {time:?}"))?;
        plan.rules.push(schedule::Rule { days, ready });
    }
    if plan.is_empty() {
        let _ = fs::remove_file(schedule_path());
    } else {
        write_public(&schedule_path(), &plan.to_text())?;
    }
    schedule_tick(true)
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
        ["set-schedule", optimized, lead, rules @ ..] => set_schedule(optimized, lead, rules),
        ["schedule-daemon"] => schedule_daemon(),
        ["charge-full", name] => charge_full(name),
        ["finish-full-charge", name] => finish_full_charge(name),
        ["restore"] => restore(),
        ["serve"] => serve(),
        _ => Err("usage: bhc-helper set-thresholds <BAT> <START|-> <END> [--persist|--no-persist]\n       \
                  bhc-helper set-behaviour <BAT> <auto|inhibit-charge|force-discharge>\n       \
                  bhc-helper charge-full <BAT>\n       \
                  bhc-helper finish-full-charge <BAT>\n       \
                  bhc-helper set-schedule <0|1> <LEAD-MINUTES> [<DAYS>@<HH:MM>]...\n       \
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
        let mut first_round = true;
        loop {
            {
                let _guard = WRITE_LOCK.lock();
                check_full_charges();
                if let Err(e) = restore() {
                    eprintln!("bhc-helper: {e}");
                }
                if let Err(e) = schedule_tick(first_round) {
                    eprintln!("bhc-helper: {e}");
                }
                first_round = false;
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
            Some("set-thresholds" | "charge-full" | "finish-full-charge" | "set-schedule") => match run_locked(&args) {
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

    pub fn data_dir() -> PathBuf {
        DIR.with(|d| d.borrow().join("data"))
    }

    pub fn config_path() -> PathBuf {
        DIR.with(|d| d.borrow().join("etc/battery-health-center.conf"))
    }

    pub fn schedule_path() -> PathBuf {
        DIR.with(|d| d.borrow().join("etc/battery-health-center.schedule"))
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

    /// A saved limit of 75-80% and a schedule that wants 100% by 08:00 every day.
    fn setup_schedule(name: &str, optimized: bool) {
        setup(name);
        fs::create_dir_all(config_path().parent().unwrap()).unwrap();
        fs::write(config_path(), "BAT0 75 80\n").unwrap();
        let rules: &[&str] = if optimized { &[] } else { &["127@08:00"] };
        set_schedule(if optimized { "1" } else { "0" }, "120", rules).unwrap();
        // set_schedule ran a tick at the real time; start from the saved limit.
        let _ = end_scheduled_full("BAT0");
        set("BAT0/charge_control_start_threshold", "75\n");
        set("BAT0/charge_control_end_threshold", "80\n");
    }

    fn clock(h: u16, m: u16) -> schedule::Clock {
        schedule::Clock { weekday: 2, minute: h * 60 + m }
    }

    #[test]
    fn schedule_lifts_the_limit_near_the_ready_time_and_puts_it_back() {
        setup_schedule("window", false);
        schedule_tick_at(1_000, clock(4, 0), false).unwrap();
        assert_eq!(limits(), ("75".into(), "80".into()), "too early");
        schedule_tick_at(1_000, clock(6, 30), false).unwrap();
        assert_eq!(limits(), ("99".into(), "100".into()));
        assert!(scheduled_marker("BAT0").exists());
        // The service's regular restore must not undo the window.
        restore().unwrap();
        assert_eq!(limits(), ("99".into(), "100".into()));
        schedule_tick_at(1_000, clock(8, 45), false).unwrap();
        assert_eq!(limits(), ("75".into(), "80".into()));
        assert!(!scheduled_marker("BAT0").exists());
    }

    #[test]
    fn removing_the_schedule_ends_a_window() {
        setup_schedule("removed", false);
        schedule_tick_at(1_000, clock(7, 0), false).unwrap();
        assert_eq!(limits(), ("99".into(), "100".into()));
        set_schedule("0", "120", &[]).unwrap();
        assert!(!schedule_path().exists());
        assert_eq!(limits(), ("75".into(), "80".into()));
    }

    #[test]
    fn one_time_full_charge_inside_a_window_keeps_the_saved_limit() {
        setup_schedule("manual-in-window", false);
        schedule_tick_at(1_000, clock(7, 0), false).unwrap();
        charge_full("BAT0").unwrap();
        assert_eq!(fs::read_to_string(full_charge_marker("BAT0")).unwrap().trim(), "75 80");
        assert!(!scheduled_marker("BAT0").exists());
        finish_full_charge("BAT0").unwrap();
        assert_eq!(limits(), ("75".into(), "80".into()));
    }

    #[test]
    fn schedule_input_is_checked() {
        setup("schedule-input");
        assert!(set_schedule("2", "120", &[]).is_err());
        assert!(set_schedule("0", "5", &["31@07:00"]).is_err());
        assert!(set_schedule("0", "120", &["0@07:00"]).is_err());
        assert!(set_schedule("0", "120", &["128@07:00"]).is_err());
        assert!(set_schedule("0", "120", &["31@25:00"]).is_err());
        assert!(set_schedule("0", "120", &["31-07:00"]).is_err());
        assert!(!schedule_path().exists());
        set_schedule("0", "90", &["31@07:30"]).unwrap();
        assert_eq!(fs::read_to_string(schedule_path()).unwrap().lines().filter(|l| !l.starts_with('#')).collect::<Vec<_>>(), ["optimized 0", "lead 90", "rule 31 07:30"]);
    }

    #[test]
    fn learns_only_from_long_plugged_in_sessions() {
        setup_schedule("learning", true);
        let hour = 3600;
        // First look while plugged in: the session starts now.
        schedule_tick_at(10 * hour, clock(0, 0), true).unwrap();
        assert!(data_dir().join("plugged-since").exists());
        // Unplugged an hour later: too short to learn from.
        set("AC/online", "0\n");
        schedule_tick_at(11 * hour, clock(1, 0), false).unwrap();
        assert!(read_unplug_log().is_empty());
        assert!(!data_dir().join("plugged-since").exists());
        // Plugged in for five hours, then unplugged: recorded.
        set("AC/online", "1\n");
        schedule_tick_at(20 * hour, clock(2, 0), false).unwrap();
        set("AC/online", "0\n");
        schedule_tick_at(25 * hour, clock(7, 0), false).unwrap();
        assert_eq!(read_unplug_log(), vec![25 * hour]);
        // An unplug seen on a process's first look may be old news.
        set("AC/online", "1\n");
        schedule_tick_at(30 * hour, clock(0, 0), false).unwrap();
        set("AC/online", "0\n");
        schedule_tick_at(40 * hour, clock(0, 0), true).unwrap();
        assert_eq!(read_unplug_log(), vec![25 * hour]);
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
