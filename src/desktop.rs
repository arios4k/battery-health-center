//! Access to GNOME's own power-related settings (the ones in Settings > Power),
//! through GSettings, plus checks for what the hardware supports.
//!
//! Every function copes with a missing schema or key, because the desktop
//! may not be GNOME.

use gtk::gio;
use gtk::glib;
use gtk::gio::prelude::*;
use crate::i18n::{tr, trnf};

pub const POWER_SCHEMA: &str = "org.gnome.settings-daemon.plugins.power";
pub const SESSION_SCHEMA: &str = "org.gnome.desktop.session";
pub const INTERFACE_SCHEMA: &str = "org.gnome.desktop.interface";

/// The settings object for a schema, or None if the schema isn't installed.
pub fn settings(schema: &str) -> Option<gio::Settings> {
    gio::SettingsSchemaSource::default()?.lookup(schema, true)?;
    Some(gio::Settings::new(schema))
}

pub fn has_key(settings: &gio::Settings, key: &str) -> bool {
    settings.settings_schema().is_some_and(|s| s.has_key(key))
}

/// The values an enum key accepts, in schema order.
pub fn enum_values(settings: &gio::Settings, key: &str) -> Vec<String> {
    let Some(schema) = settings.settings_schema().filter(|s| s.has_key(key)) else {
        return Vec::new();
    };
    // The range is ("enum", <array of values>).
    let range = schema.key(key).range();
    if range.n_children() != 2 || range.child_value(0).str() != Some("enum") {
        return Vec::new();
    }
    let values = range.child_value(1);
    let values = values.as_variant().unwrap_or(values);
    values.iter().filter_map(|v| v.str().map(String::from)).collect()
}

/// A whole-number key (seconds) that may be stored as int32 or uint32.
pub fn get_secs(settings: &gio::Settings, key: &str) -> i64 {
    let v = settings.value(key);
    v.get::<i32>().map(i64::from).or_else(|| v.get::<u32>().map(i64::from)).unwrap_or(0)
}

pub fn set_secs(settings: &gio::Settings, key: &str, secs: i64) -> bool {
    let current = settings.value(key);
    let value = if current.is::<i32>() {
        (secs.clamp(0, i32::MAX as i64) as i32).to_variant()
    } else {
        (secs.clamp(0, u32::MAX as i64) as u32).to_variant()
    };
    settings.set_value(key, &value).is_ok()
}

/// The suspend-when-inactive delay in seconds for a "<prefix>-type" /
/// "<prefix>-timeout" pair of keys; None means never.
pub fn suspend_delay(settings: &gio::Settings, prefix: &str) -> Option<i64> {
    let secs = get_secs(settings, &format!("{prefix}-timeout"));
    (settings.string(&format!("{prefix}-type")) != "nothing" && secs > 0).then_some(secs)
}

/// Sets the delay (None = never). Choosing "never" keeps the stored delay so
/// the earlier value is still there if it's switched back on elsewhere.
pub fn set_suspend_delay(settings: &gio::Settings, prefix: &str, choice: Option<i64>) -> bool {
    match choice {
        None => settings.set_string(&format!("{prefix}-type"), "nothing").is_ok(),
        Some(secs) => {
            set_secs(settings, &format!("{prefix}-timeout"), secs)
                && settings.set_string(&format!("{prefix}-type"), "suspend").is_ok()
        }
    }
}

/// Whether the laptop can hibernate (logind says "yes"). Settings only lists
/// Hibernate when it works.
pub fn can_hibernate() -> bool {
    let Ok(proxy) = gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS | gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES,
        None,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        gio::Cancellable::NONE,
    ) else {
        return false;
    };
    proxy
        .call_sync("CanHibernate", None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE)
        .ok()
        .and_then(|r| r.child_value(0).str().map(|s| s == "yes"))
        .unwrap_or(false)
}

/// What could save battery right now: screen and keyboard brightness (through
/// GNOME's power daemon) and Bluetooth (through BlueZ). Each part is None when
/// it can't be read, for example when there is no Bluetooth adapter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavingState {
    /// Percent.
    pub screen: Option<u8>,
    pub keyboard: Option<u8>,
    pub bluetooth: Option<Bluetooth>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bluetooth {
    /// D-Bus path of the adapter, e.g. /org/bluez/hci0.
    pub adapter: String,
    pub powered: bool,
    pub connected_devices: usize,
}

const POWER_NAME: &str = "org.gnome.SettingsDaemon.Power";
const POWER_PATH: &str = "/org/gnome/SettingsDaemon/Power";
const SCREEN_IFACE: &str = "org.gnome.SettingsDaemon.Power.Screen";
const KEYBOARD_IFACE: &str = "org.gnome.SettingsDaemon.Power.Keyboard";

fn power_proxy(iface: &str) -> Option<gio::DBusProxy> {
    gio::DBusProxy::for_bus_sync(
        gio::BusType::Session,
        gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
        None,
        POWER_NAME,
        POWER_PATH,
        iface,
        gio::Cancellable::NONE,
    )
    .ok()
}

fn read_brightness(iface: &str) -> Option<u8> {
    let percent = power_proxy(iface)?.cached_property("Brightness")?.get::<i32>()?;
    u8::try_from(percent).ok().filter(|p| *p <= 100)
}

fn write_brightness(iface: &str, percent: u8) -> bool {
    let Some(proxy) = power_proxy(iface) else { return false };
    let args = (iface, "Brightness", i32::from(percent).to_variant()).to_variant();
    proxy.call_sync("org.freedesktop.DBus.Properties.Set", Some(&args), gio::DBusCallFlags::NONE, 1000, gio::Cancellable::NONE).is_ok()
}

fn bluez_proxy(path: &str, iface: &str) -> Option<gio::DBusProxy> {
    gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS | gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES,
        None,
        "org.bluez",
        path,
        iface,
        gio::Cancellable::NONE,
    )
    .ok()
}

fn read_bluetooth() -> Option<Bluetooth> {
    let reply = bluez_proxy("/", "org.freedesktop.DBus.ObjectManager")?
        .call_sync("GetManagedObjects", None, gio::DBusCallFlags::NONE, 1000, gio::Cancellable::NONE)
        .ok()?;
    let objects = reply.child_value(0);
    let mut adapter = None;
    let mut connected = 0;
    for i in 0..objects.n_children() {
        let entry = objects.child_value(i);
        let path = entry.child_value(0).str()?.to_string();
        let interfaces = entry.child_value(1);
        for j in 0..interfaces.n_children() {
            let iface = interfaces.child_value(j);
            let props = glib::VariantDict::new(Some(&iface.child_value(1)));
            let flag = |name: &str| props.lookup_value(name, None).and_then(|v| v.get::<bool>()).unwrap_or(false);
            match iface.child_value(0).str() {
                Some("org.bluez.Adapter1") => {
                    adapter.get_or_insert((path.clone(), flag("Powered")));
                }
                Some("org.bluez.Device1") if flag("Connected") => connected += 1,
                _ => {}
            }
        }
    }
    let (adapter, powered) = adapter?;
    Some(Bluetooth { adapter, powered, connected_devices: connected })
}

thread_local! {
    /// The last reading and when it was taken; reading asks other programs
    /// over D-Bus, so it's reused for a while.
    static SAVING: std::cell::RefCell<Option<(std::time::Instant, SavingState)>> = const { std::cell::RefCell::new(None) };
}

const SAVING_TTL: std::time::Duration = std::time::Duration::from_secs(20);

pub fn saving_state() -> SavingState {
    SAVING.with(|cache| {
        if let Some((at, state)) = &*cache.borrow() {
            if at.elapsed() < SAVING_TTL {
                return state.clone();
            }
        }
        let state = SavingState { screen: read_brightness(SCREEN_IFACE), keyboard: read_brightness(KEYBOARD_IFACE), bluetooth: read_bluetooth() };
        *cache.borrow_mut() = Some((std::time::Instant::now(), state.clone()));
        state
    })
}

/// Forget the last reading, after changing something.
fn forget_saving_state() {
    SAVING.with(|cache| *cache.borrow_mut() = None);
}

pub fn set_screen_brightness(percent: u8) -> bool {
    let ok = write_brightness(SCREEN_IFACE, percent);
    forget_saving_state();
    ok
}

pub fn set_keyboard_brightness(percent: u8) -> bool {
    let ok = write_brightness(KEYBOARD_IFACE, percent);
    forget_saving_state();
    ok
}

pub fn set_bluetooth_power(adapter: &str, on: bool) -> bool {
    forget_saving_state();
    let Some(proxy) = bluez_proxy(adapter, "org.freedesktop.DBus.Properties") else { return false };
    let args = ("org.bluez.Adapter1", "Powered", on.to_variant()).to_variant();
    proxy.call_sync("Set", Some(&args), gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE).is_ok()
}

/// Whether an ambient light sensor is available (needed for automatic brightness).
pub fn has_ambient_light() -> bool {
    gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
        None,
        "net.hadess.SensorProxy",
        "/net/hadess/SensorProxy",
        "net.hadess.SensorProxy",
        gio::Cancellable::NONE,
    )
    .ok()
    .and_then(|p| p.cached_property("HasAmbientLight"))
    .and_then(|v| v.get::<bool>())
    .unwrap_or(false)
}

pub fn action_label(value: &str) -> String {
    match value {
        "suspend" => tr("Suspend").into(),
        "hibernate" => tr("Hibernate").into(),
        "nothing" => tr("Do Nothing").into(),
        "interactive" => tr("Ask What to Do").into(),
        "blank" => tr("Turn Off the Screen").into(),
        "shutdown" => tr("Shut Down").into(),
        "logout" => tr("Log Out").into(),
        other => {
            let mut c = other.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        }
    }
}

/// "5 minutes", "1 hour", "90 minutes".
pub fn duration_label(secs: i64) -> String {
    let count = |n: i64| [("n", n.to_string())];
    match secs {
        s if s < 60 => trnf("{n} second", "{n} seconds", s as u64, &count(s)),
        s if s < 3600 => trnf("{n} minute", "{n} minutes", (s / 60) as u64, &count(s / 60)),
        s if s % 3600 == 0 => trnf("{n} hour", "{n} hours", (s / 3600) as u64, &count(s / 3600)),
        s => trnf("{n} minute", "{n} minutes", (s / 60) as u64, &count(s / 60)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settings backed by memory, so tests never touch the real desktop
    /// settings. None if the schema isn't installed (then the test is skipped).
    fn memory_settings(schema: &str) -> Option<gio::Settings> {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| std::env::set_var("GSETTINGS_BACKEND", "memory"));
        settings(schema)
    }

    #[test]
    fn suspend_delay_roundtrip() {
        let Some(s) = memory_settings(POWER_SCHEMA) else { return };
        assert!(set_suspend_delay(&s, "sleep-inactive-battery", Some(1800)));
        assert_eq!(suspend_delay(&s, "sleep-inactive-battery"), Some(1800));
        assert_eq!(s.string("sleep-inactive-battery-type"), "suspend");
        assert!(set_suspend_delay(&s, "sleep-inactive-battery", None));
        assert_eq!(suspend_delay(&s, "sleep-inactive-battery"), None);
        assert_eq!(s.string("sleep-inactive-battery-type"), "nothing");
        // The delay itself is kept, so turning it back on restores a sensible value.
        assert_eq!(get_secs(&s, "sleep-inactive-battery-timeout"), 1800);
        assert!(set_suspend_delay(&s, "sleep-inactive-battery", Some(900)));
        assert_eq!(suspend_delay(&s, "sleep-inactive-battery"), Some(900));
    }

    #[test]
    fn seconds_keys_keep_their_type() {
        // The session key is unsigned, the power ones signed.
        if let Some(s) = memory_settings(SESSION_SCHEMA) {
            assert!(set_secs(&s, "idle-delay", 480));
            assert_eq!(get_secs(&s, "idle-delay"), 480);
            assert!(s.value("idle-delay").is::<u32>());
        }
        if let Some(s) = memory_settings(POWER_SCHEMA) {
            assert!(set_secs(&s, "sleep-inactive-ac-timeout", 2700));
            assert_eq!(get_secs(&s, "sleep-inactive-ac-timeout"), 2700);
            assert!(s.value("sleep-inactive-ac-timeout").is::<i32>());
        }
    }

    #[test]
    fn enum_values_come_from_the_schema() {
        let Some(s) = memory_settings(POWER_SCHEMA) else { return };
        let lid = enum_values(&s, "lid-close-battery-action");
        assert!(lid.contains(&"suspend".to_string()) && lid.contains(&"nothing".to_string()), "{lid:?}");
        assert!(enum_values(&s, "no-such-key").is_empty());
        assert!(!has_key(&s, "no-such-key"));
    }

    #[test]
    fn labels() {
        assert_eq!(duration_label(60), "1 minute");
        assert_eq!(duration_label(300), "5 minutes");
        assert_eq!(duration_label(3600), "1 hour");
        assert_eq!(duration_label(7200), "2 hours");
        assert_eq!(duration_label(5400), "90 minutes");
        assert_eq!(action_label("nothing"), "Do Nothing");
        assert_eq!(action_label("weird"), "Weird");
    }
}
