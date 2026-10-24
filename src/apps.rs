//! Estimates which apps are using the most energy.
//!
//! Linux has no per-app battery accounting, so (like macOS's "Energy Impact")
//! this is a heuristic: CPU time is sampled from /proc and processes are grouped
//! into apps using the systemd scope GNOME launches each app in
//! (e.g. `app-gnome-firefox-1234.scope`), then resolved to a .desktop entry for
//! a friendly name and icon. When running on battery, the measured system power
//! above the idle floor is split between apps by their share of CPU time.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::rc::Rc;

use gtk::gio;
use gtk::gio::prelude::*;

#[derive(Clone, Debug)]
pub struct AppUsage {
    pub name: String,
    pub icon: Option<gio::Icon>,
    pub system: bool,
    pub processes: usize,
    /// Smoothed CPU usage in percent of one core.
    pub impact: f64,
    /// Estimated power draw in watts (only while on battery).
    pub watts: Option<f64>,
}

#[derive(Debug)]
struct GroupInfo {
    key: String,
    name: String,
    icon: Option<gio::Icon>,
    system: bool,
}

struct ProcStat {
    comm: String,
    ppid: i32,
    ticks: u64,
    start: u64,
}

fn read_stat(pid: i32) -> Option<ProcStat> {
    let s = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let open = s.find('(')?;
    let close = s.rfind(')')?;
    let comm = s[open + 1..close].to_string();
    let f: Vec<&str> = s.get(close + 2..)?.split_whitespace().collect();
    // f[0] is field 3 (state); utime=14, stime=15, starttime=22
    let num = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok());
    Some(ProcStat {
        comm,
        ppid: f.get(1)?.parse().ok()?,
        ticks: num(11)? + num(12)?,
        start: num(19)?,
    })
}

fn total_cpu_ticks() -> Option<u64> {
    let s = fs::read_to_string("/proc/stat").ok()?;
    let line = s.lines().next()?;
    // user nice system idle iowait irq softirq steal (guest is already in user)
    Some(line.split_whitespace().skip(1).take(8).filter_map(|v| v.parse::<u64>().ok()).sum())
}

fn unescape_unit(s: &str) -> String {
    s.replace("\\x2d", "-").replace("\\x2e", ".").replace("\\x5f", "_")
}

/// Candidate application ids extracted from a systemd unit name such as
/// `app-gnome-org.gnome.Nautilus-1234.scope`, `app-com.anthropic.Claude-99.scope`,
/// `app-flatpak-org.mozilla.firefox-5678.scope`, `app-dbus-:1.2-org.foo@0.service`
/// or `snap.firefox.firefox-<uuid>.scope`.
fn app_id_candidates(unit: &str) -> Vec<String> {
    let base = unit
        .trim_end_matches(".scope")
        .trim_end_matches(".service")
        .trim_end_matches(".slice");
    let base = base.split('@').next().unwrap_or(base);

    if let Some(rest) = base.strip_prefix("snap.") {
        let mut it = rest.split('.');
        let snap = it.next().unwrap_or_default();
        let app = it.next().unwrap_or_default().split('-').next().unwrap_or_default();
        return vec![format!("{snap}_{app}"), snap.to_string()];
    }

    let Some(rest) = base.strip_prefix("app-") else {
        return Vec::new();
    };
    const LAUNCHERS: &[&str] = &["gnome", "glib", "flatpak", "kde", "dbus", "systemd", "xdg", "autostart"];
    rest.split('-')
        .filter(|p| !p.is_empty())
        .filter(|p| !LAUNCHERS.contains(p))
        .filter(|p| !p.starts_with(':'))
        .filter(|p| !p.chars().all(|c| c.is_ascii_hexdigit()))
        .map(unescape_unit)
        .collect()
}

fn pretty_process_name(comm: &str) -> String {
    let known = match comm {
        "gnome-shell" => "GNOME Shell",
        "Xwayland" => "Xwayland",
        "pipewire" | "pipewire-pulse" | "wireplumber" => "Audio (PipeWire)",
        "tracker-miner-f" | "localsearch-3" => "File Indexer",
        "gnome-software" => "Software",
        "packagekitd" => "Software Updates",
        "Xorg" => "Display Server (Xorg)",
        "systemd" => "systemd",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_string();
    }
    let mut c = comm.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => comm.to_string(),
    }
}

pub struct AppSampler {
    ncpu: f64,
    prev_total: Option<u64>,
    prev_procs: HashMap<i32, (u64, u64)>, // pid -> (start, ticks)
    groups: HashMap<(i32, u64), Rc<GroupInfo>>,
    /// Installed apps by lowercase desktop id (without ".desktop").
    desktop_apps: HashMap<String, (String, Option<gio::Icon>)>,
    smoothed: HashMap<String, f64>,
    cumulative: HashMap<String, (f64, Rc<GroupInfo>)>,
    samples: u64,
}

impl AppSampler {
    pub fn new() -> Self {
        AppSampler {
            ncpu: std::thread::available_parallelism().map(|n| n.get() as f64).unwrap_or(1.0),
            prev_total: None,
            prev_procs: HashMap::new(),
            groups: HashMap::new(),
            desktop_apps: gio::AppInfo::all()
                .into_iter()
                .filter_map(|info| {
                    let id = info.id()?.trim_end_matches(".desktop").to_lowercase();
                    Some((id, (info.display_name().to_string(), info.icon())))
                })
                .collect(),
            smoothed: HashMap::new(),
            cumulative: HashMap::new(),
            samples: 0,
        }
    }

    fn lookup_desktop(&self, id: &str) -> Option<(String, Option<gio::Icon>)> {
        self.desktop_apps.get(&id.to_lowercase()).cloned()
    }

    fn classify(&mut self, pid: i32, stat: &ProcStat) -> Rc<GroupInfo> {
        self.classify_with_depth(pid, stat, 0)
    }

    fn classify_with_depth(&mut self, pid: i32, stat: &ProcStat, depth: u32) -> Rc<GroupInfo> {
        if let Some(g) = self.groups.get(&(pid, stat.start)) {
            return g.clone();
        }
        let info = self.classify_own(pid, stat);
        // Some apps move their processes out of the scope GNOME launched them
        // in (e.g. Chromium-based browsers), so if this process doesn't map to
        // an app itself, attribute it to the nearest ancestor that does.
        let info = if info.key.starts_with("app:") || info.key == "kernel" || stat.ppid <= 1 || depth >= 32 {
            info
        } else {
            match read_stat(stat.ppid) {
                Some(parent) => {
                    let p = self.classify_with_depth(stat.ppid, &parent, depth + 1);
                    if p.key.starts_with("app:") { p } else { info }
                }
                None => info,
            }
        };
        self.groups.insert((pid, stat.start), info.clone());
        info
    }

    /// Classifies a process from its own cgroup and name only.
    fn classify_own(&mut self, pid: i32, stat: &ProcStat) -> Rc<GroupInfo> {
        let cgroup = fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap_or_default();
        let path = cgroup.lines().find_map(|l| l.strip_prefix("0::")).unwrap_or("/").to_string();

        let info = if stat.ppid == 2 || pid == 2 {
            GroupInfo {
                key: "kernel".into(),
                name: "Linux Kernel".into(),
                icon: Some(gio::ThemedIcon::new("computer-symbolic").upcast()),
                system: true,
            }
        } else {
            // Look for the innermost app unit in the cgroup path; processes
            // started inside an app (e.g. a shell in Terminal) count toward it.
            let app = path
                .rsplit('/')
                .filter(|c| c.starts_with("app-") || c.starts_with("snap."))
                .flat_map(app_id_candidates)
                .find_map(|id| self.lookup_desktop(&id).map(|d| (id, d)));
            match app {
                Some((id, (name, icon))) => GroupInfo { key: format!("app:{id}"), name, icon, system: false },
                None => {
                    let system = !path.starts_with("/user.slice");
                    let desktop = self.lookup_desktop(&stat.comm);
                    let (name, icon) = desktop.unwrap_or_else(|| (pretty_process_name(&stat.comm), None));
                    GroupInfo { key: format!("proc:{}", stat.comm), name, icon, system }
                }
            }
        };
        Rc::new(info)
    }

    /// Takes a new sample. `dynamic_power_w` is the measured system power
    /// above the idle floor (only known while on battery).
    pub fn sample(&mut self, dynamic_power_w: Option<f64>) -> Vec<AppUsage> {
        let Some(total) = total_cpu_ticks() else {
            return Vec::new();
        };
        let first = self.prev_total.is_none();
        let total_delta = total.saturating_sub(self.prev_total.unwrap_or(total)).max(1) as f64;
        self.prev_total = Some(total);

        let mut per_group: HashMap<String, (u64, usize, Rc<GroupInfo>)> = HashMap::new();
        let mut seen = HashSet::new();
        let pids: Vec<i32> = fs::read_dir("/proc")
            .map(|d| d.flatten().filter_map(|e| e.file_name().to_str()?.parse().ok()).collect())
            .unwrap_or_default();

        for pid in pids {
            let Some(stat) = read_stat(pid) else { continue };
            seen.insert(pid);
            let delta = match self.prev_procs.insert(pid, (stat.start, stat.ticks)) {
                Some((start, prev)) if start == stat.start => stat.ticks.saturating_sub(prev),
                // New process since the last sample: all its CPU time is recent.
                _ if !first => stat.ticks,
                _ => 0,
            };
            let info = self.classify(pid, &stat);
            let entry = per_group.entry(info.key.clone()).or_insert((0, 0, info));
            entry.0 += delta;
            entry.1 += 1;
        }
        self.prev_procs.retain(|pid, _| seen.contains(pid));
        self.groups.retain(|(pid, _), _| seen.contains(pid));
        if first {
            return Vec::new();
        }
        self.samples += 1;

        // Percent of one core, like `top`.
        let core_ticks = total_delta / self.ncpu;
        let mut current: HashMap<String, f64> = HashMap::new();
        for (key, (delta, _, info)) in &per_group {
            let pct = *delta as f64 / core_ticks * 100.0;
            current.insert(key.clone(), pct);
            let c = self.cumulative.entry(key.clone()).or_insert((0.0, info.clone()));
            c.0 += pct;
            c.1 = info.clone();
        }
        // Exponential smoothing so the list doesn't jump around.
        let keys: HashSet<String> = self.smoothed.keys().chain(current.keys()).cloned().collect();
        for key in keys {
            let cur = current.get(&key).copied().unwrap_or(0.0);
            let prev = self.smoothed.get(&key).copied().unwrap_or(cur);
            let v = prev * 0.5 + cur * 0.5;
            if v < 0.05 && !current.contains_key(&key) {
                self.smoothed.remove(&key);
            } else {
                self.smoothed.insert(key, v);
            }
        }

        let sum: f64 = self.smoothed.values().sum::<f64>().max(0.001);
        let mut out: Vec<AppUsage> = per_group
            .into_iter()
            .map(|(key, (_, processes, info))| {
                let impact = self.smoothed.get(&key).copied().unwrap_or(0.0);
                AppUsage {
                    name: info.name.clone(),
                    icon: info.icon.clone(),
                    system: info.system,
                    processes,
                    impact,
                    watts: dynamic_power_w.map(|p| p * impact / sum),
                }
            })
            .collect();
        out.sort_by(|a, b| b.impact.total_cmp(&a.impact));
        out
    }

    /// Average CPU impact per app since the sampler started.
    pub fn averages(&self) -> Vec<AppUsage> {
        let n = self.samples.max(1) as f64;
        let mut out: Vec<AppUsage> = self
            .cumulative
            .iter()
            .map(|(_, (sum, info))| AppUsage {
                name: info.name.clone(),
                icon: info.icon.clone(),
                system: info.system,
                processes: 0,
                impact: sum / n,
                watts: None,
            })
            .collect();
        out.sort_by(|a, b| b.impact.total_cmp(&a.impact));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_parsing() {
        assert_eq!(app_id_candidates("app-gnome-org.gnome.Nautilus-1234.scope"), vec!["org.gnome.Nautilus"]);
        assert_eq!(app_id_candidates("app-com.anthropic.Claude-2375.scope"), vec!["com.anthropic.Claude"]);
        assert_eq!(app_id_candidates("app-flatpak-org.mozilla.firefox-5678.scope"), vec!["org.mozilla.firefox"]);
        assert_eq!(app_id_candidates("app-dbus-:1.2-org.gnome.Calculator@0.service"), vec!["org.gnome.Calculator"]);
        assert_eq!(app_id_candidates("app-org.gnome.Terminal.slice"), vec!["org.gnome.Terminal"]);
        assert_eq!(app_id_candidates("app-gnome-google\\x2dchrome-999.scope"), vec!["google-chrome"]);
        assert_eq!(app_id_candidates("snap.firefox.firefox-abcd.scope")[0], "firefox_firefox");
    }
}
