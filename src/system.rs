//! Integration with the rest of the system: the privileged helper, the
//! power-profiles daemon, and login autostart.

use std::path::PathBuf;
use std::process::Command;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib::prelude::ToVariant;
use gtk::glib;

use crate::APP_ID;

pub const HELPER_CONFIG: &str = "/etc/battery-health-center.conf";

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

/// Runs the helper as root through pkexec without blocking the UI.
pub async fn run_helper(args: Vec<String>) -> Result<(), String> {
    let helper = helper_path().ok_or("The bhc-helper program was not found. Is the app installed?")?;
    let output = gio::spawn_blocking(move || Command::new("pkexec").arg(helper).args(args).output())
        .await
        .map_err(|_| "The helper crashed".to_string())?
        .map_err(|e| format!("Could not run pkexec: {e}"))?;
    match output.status.code() {
        Some(0) => Ok(()),
        Some(126) | Some(127) => Err("Authorization was cancelled".into()),
        _ => {
            let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if msg.is_empty() { "The change could not be applied".into() } else { msg })
        }
    }
}

/// Whether the charge limit for this battery is saved to be restored at boot.
pub fn limit_persisted(battery: &str) -> bool {
    std::fs::read_to_string(HELPER_CONFIG)
        .map(|c| c.lines().any(|l| l.split_whitespace().next() == Some(battery)))
        .unwrap_or(false)
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
            .map(|_| ())
    }
}

pub fn profile_label(profile: &str) -> &str {
    match profile {
        "power-saver" => "Power Saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
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
