//! Integration with the rest of the system: the privileged helper, the
//! power-profiles daemon, and login autostart.

use std::path::PathBuf;
use std::process::Command;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib::prelude::ToVariant;
use gtk::glib;

use crate::APP_ID;

/// Interfaces the snap needs, with what each one is for.
pub const SNAP_PLUGS: &[(&str, &str)] = &[
    ("hardware-observe", "read battery information"),
    ("system-observe", "see which apps use energy"),
    ("power-control", "change the charge limit"),
];

pub fn in_snap() -> bool {
    std::env::var_os("SNAP").is_some()
}

/// Snap interfaces that are not connected yet (empty outside the snap).
pub fn missing_snap_plugs() -> Vec<&'static str> {
    if !in_snap() {
        return Vec::new();
    }
    SNAP_PLUGS
        .iter()
        .map(|(plug, _)| *plug)
        .filter(|plug| !Command::new("snapctl").args(["is-connected", plug]).status().is_ok_and(|s| s.success()))
        .collect()
}


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

/// Sends a request to the snap's helper service (see bhc-helper `serve`).
fn request_snap_service(args: &[String]) -> Result<(), String> {
    use std::io::{BufRead, BufReader, Write};
    let dir = std::env::var_os("SNAP_COMMON").ok_or("SNAP_COMMON is not set")?;
    let socket = PathBuf::from(dir).join("helper.sock");
    let mut stream = std::os::unix::net::UnixStream::connect(&socket)
        .map_err(|_| "The Battery Health Center service is not running".to_string())?;
    stream.write_all(format!("{}\n", args.join(" ")).as_bytes()).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(|e| e.to_string())?;
    match reply.trim() {
        "OK" => Ok(()),
        r if r.contains("Permission denied") => Err(
            "Permission denied. Run: snap connect battery-health-center:power-control".to_string(),
        ),
        r => Err(r.strip_prefix("ERR ").unwrap_or(r).to_string()),
    }
}

/// Runs the helper as root without blocking the UI: through the snap's
/// service when sandboxed, otherwise through pkexec.
pub async fn run_helper(args: Vec<String>) -> Result<(), String> {
    if in_snap() {
        return gio::spawn_blocking(move || request_snap_service(&args))
            .await
            .map_err(|_| "The helper crashed".to_string())?;
    }
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


pub struct PowerProfiles {
    proxy: gio::DBusProxy,
    interface: &'static str,
}

impl PowerProfiles {
    pub fn connect() -> Option<Self> {
        // No snap interface grants access to power-profiles-daemon.
        if in_snap() {
            return None;
        }
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
    // In the snap, snapd starts the app named by the `autostart` key and only
    // takes the arguments from Exec.
    let exe = if in_snap() { PathBuf::from("battery-health-center") } else { std::env::current_exe()? };
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
