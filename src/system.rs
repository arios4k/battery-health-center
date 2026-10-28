//! Integration with the rest of the system: the privileged helper, the
//! power-profiles daemon, and login autostart.

use std::path::PathBuf;
use std::process::Command;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib::prelude::ToVariant;
use gtk::glib;

use crate::APP_ID;
use crate::schedule::{self, Learned, Schedule};
use crate::i18n::{tr, trf};

/// Locates bhc-helper: the installed copy (whose path the polkit policy
/// names), or the one next to this binary when running from the build tree.
pub fn helper_path() -> Option<PathBuf> {
    let installed = [
        "/usr/libexec/battery-health-center/bhc-helper",
        "/usr/local/libexec/battery-health-center/bhc-helper",
    ];
    installed
        .iter()
        .map(PathBuf::from)
        .chain(std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join("bhc-helper"))))
        .find(|p| p.is_file())
}

/// Runs the helper as root through pkexec, without blocking the UI.
pub async fn run_helper(args: Vec<String>) -> Result<(), String> {
    let helper = helper_path().ok_or(tr("The bhc-helper program was not found. Is the app installed?"))?;
    let output = gio::spawn_blocking(move || Command::new("pkexec").arg(helper).args(args).output())
        .await
        .map_err(|_| tr("The helper crashed").to_string())?
        .map_err(|e| trf("Could not run pkexec: {e}", &[("e", e.to_string())]))?;
    match output.status.code() {
        Some(0) => Ok(()),
        Some(126) | Some(127) => Err(tr("Authorization was cancelled").into()),
        _ => {
            let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if msg.is_empty() { tr("The change could not be applied").into() } else { msg })
        }
    }
}


pub struct PowerProfiles {
    proxy: gio::DBusProxy,
    interface: &'static str,
}

impl PowerProfiles {
    pub fn connect() -> Option<Self> {
        let candidates = [
            ("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles"),
            ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
        ];
        candidates.into_iter().find_map(|(name, path)| {
            let proxy = gio::DBusProxy::for_bus_sync(
                gio::BusType::System,
                gio::DBusProxyFlags::NONE,
                None,
                name,
                path,
                name,
                gio::Cancellable::NONE,
            )
            .ok()?;
            proxy.name_owner()?;
            Some(PowerProfiles { proxy, interface: name })
        })
    }

    pub fn active(&self) -> Option<String> {
        self.proxy.cached_property("ActiveProfile")?.str().map(String::from)
    }

    pub fn available(&self) -> Vec<String> {
        let Some(profiles) = self.proxy.cached_property("Profiles") else {
            return Vec::new();
        };
        profiles
            .iter()
            .filter_map(|p| glib::VariantDict::new(Some(&p)).lookup_value("Profile", None)?.str().map(String::from))
            .collect()
    }

    pub fn set_active(&self, profile: &str) -> Result<(), glib::Error> {
        let args = (self.interface, "ActiveProfile", profile.to_variant()).to_variant();
        self.proxy
            .call_sync(
                "org.freedesktop.DBus.Properties.Set",
                Some(&args),
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
            )
            .map(|_| ())?;
        // The daemon announces the change in a signal that is handled later; until
        // then the cached value would still be the old mode, and a page refresh
        // would wrongly put the old mode back on screen.
        self.proxy.set_cached_property("ActiveProfile", Some(&profile.to_variant()));
        Ok(())
    }
}

pub fn profile_label(profile: &str) -> &str {
    match profile {
        "power-saver" => tr("Power Saver"),
        "balanced" => tr("Balanced"),
        "performance" => tr("Performance"),
        other => other,
    }
}

fn autostart_file() -> PathBuf {
    glib::user_config_dir().join("autostart").join(format!("{APP_ID}.desktop"))
}

pub fn autostart_enabled() -> bool {
    autostart_file().is_file()
}

pub fn set_autostart(enabled: bool) -> std::io::Result<()> {
    let path = autostart_file();
    if !enabled {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let exe = std::env::current_exe()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        path,
        format!(
            "[Desktop Entry]\nType=Application\nName=Battery Health Center\n\
             Comment=Records battery history in the background\n\
             Exec=\"{}\" --background\nIcon={APP_ID}\nNoDisplay=true\n\
             X-GNOME-Autostart-enabled=true\n",
            exe.display()
        ),
    )
}

/// Whether a one-time "charge to 100%" is in progress for this battery (the
/// helper keeps a marker file while it is).
pub fn full_charge_active(battery: &str) -> bool {
    PathBuf::from("/run/battery-health-center").join(format!("full-charge-{battery}")).exists()
}

/// The helper's files for the saved limit and the schedule, in /etc and
/// /var/lib. All world-readable. `BHC_HELPER_FILES=<folder>` reads them from a
/// folder instead (under their file names), to try the schedule screens
/// without touching the real settings.
fn helper_file(name: &str, system_path: &str) -> PathBuf {
    match std::env::var_os("BHC_HELPER_FILES") {
        Some(dir) => PathBuf::from(dir).join(name),
        None => PathBuf::from(system_path),
    }
}

/// Whether a charge limit below 100% is saved to be re-applied (the schedule
/// lifts and restores that saved limit).
pub fn has_saved_limit() -> bool {
    let config = std::fs::read_to_string(helper_file("battery-health-center.conf", "/etc/battery-health-center.conf")).unwrap_or_default();
    config.lines().filter(|l| !l.starts_with('#')).any(|l| l.split_whitespace().nth(2).and_then(|e| e.parse::<u8>().ok()).is_some_and(|e| e < 100))
}

/// The saved charging schedule (empty when there is none).
pub fn read_schedule() -> Schedule {
    let text = std::fs::read_to_string(helper_file("battery-health-center.schedule", "/etc/battery-health-center.schedule")).unwrap_or_default();
    Schedule::parse(&text)
}

/// What optimized charging has learned so far from when you unplug.
pub fn read_learned() -> Learned {
    let text = std::fs::read_to_string(helper_file("unplug-log", "/var/lib/battery-health-center/unplug-log")).unwrap_or_default();
    let unplugs: Vec<i64> = text.lines().filter_map(|l| l.trim().parse().ok()).collect();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    schedule::learn(&unplugs, now)
}

/// Whether the schedule has lifted the limit for a charge to 100% right now.
pub fn scheduled_full_active(battery: &str) -> bool {
    PathBuf::from("/run/battery-health-center").join(format!("scheduled-full-{battery}")).exists()
}

/// A battery-powered peripheral reported by UPower (mouse, headphones, ...).
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceBattery {
    pub name: String,
    pub icon: &'static str,
    /// Exact percentage, if the device reports one.
    pub percent: Option<f64>,
    /// Coarse level for devices that only report e.g. "Low" or "Full".
    pub level: Option<&'static str>,
}

fn device_icon(kind: u32) -> &'static str {
    // UpDeviceKind values from UPower.
    match kind {
        5 => "input-mouse-symbolic",
        6 => "input-keyboard-symbolic",
        8 => "phone-symbolic",
        10 | 13 => "input-tablet-symbolic",
        12 => "input-gaming-symbolic",
        14 => "input-touchpad-symbolic",
        17 => "audio-headset-symbolic",
        18 => "audio-speakers-symbolic",
        19 => "audio-headphones-symbolic",
        23 => "printer-symbolic",
        25 => "camera-photo-symbolic",
        _ => "bluetooth-active-symbolic",
    }
}

fn device_level(level: u32) -> Option<&'static str> {
    match level {
        3 => Some(tr("Low")),
        4 => Some(tr("Critical")),
        6 => Some(tr("Normal")),
        7 => Some(tr("High")),
        8 => Some(tr("Full")),
        _ => None,
    }
}

/// Peripheral batteries from UPower. Laptop batteries and power supplies
/// are excluded (they have PowerSupply set).
pub fn device_batteries() -> Vec<DeviceBattery> {
    let bus = |path: &str, iface: &str| {
        gio::DBusProxy::for_bus_sync(
            gio::BusType::System,
            gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
            None,
            "org.freedesktop.UPower",
            path,
            iface,
            gio::Cancellable::NONE,
        )
        .ok()
    };
    let Some(upower) = bus("/org/freedesktop/UPower", "org.freedesktop.UPower") else {
        return Vec::new();
    };
    let Ok(reply) =
        upower.call_sync("EnumerateDevices", None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE)
    else {
        return Vec::new();
    };
    let paths: Vec<String> = reply.child_value(0).iter().filter_map(|p| p.str().map(String::from)).collect();
    paths
        .iter()
        .filter_map(|path| {
            let dev = bus(path, "org.freedesktop.UPower.Device")?;
            let prop = |name: &str| dev.cached_property(name);
            let power_supply = prop("PowerSupply")?.get::<bool>()?;
            let present = prop("IsPresent").and_then(|v| v.get::<bool>()).unwrap_or(true);
            let kind = prop("Type")?.get::<u32>()?;
            // 0 unknown, 1 line power, 2 laptop battery
            if power_supply || !present || kind <= 2 {
                return None;
            }
            let model = prop("Model").and_then(|v| v.str().map(String::from)).filter(|m| !m.is_empty());
            let level = prop("BatteryLevel").and_then(|v| v.get::<u32>()).and_then(device_level);
            let percent = prop("Percentage").and_then(|v| v.get::<f64>()).filter(|p| *p > 0.0 && level.is_none());
            Some(DeviceBattery {
                name: model.unwrap_or_else(|| tr("Wireless Device").into()),
                icon: device_icon(kind),
                percent,
                level,
            })
        })
        .collect()
}
