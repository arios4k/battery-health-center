//! User preferences and small bits of persistent state, stored as key=value
//! lines in ~/.config/battery-health-center/settings.ini.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Steps of the guided calibration (see ui::health).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Calibration {
    #[default]
    Off,
    /// Charge to 100% while plugged in.
    ChargeFull,
    /// Unplug and use the laptop until the battery is nearly empty.
    Discharge,
    /// Plug in and charge to 100% again.
    Recharge,
}

impl Calibration {
    fn as_str(self) -> &'static str {
        match self {
            Calibration::Off => "off",
            Calibration::ChargeFull => "charge-full",
            Calibration::Discharge => "discharge",
            Calibration::Recharge => "recharge",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "charge-full" => Calibration::ChargeFull,
            "discharge" => Calibration::Discharge,
            "recharge" => Calibration::Recharge,
            _ => Calibration::Off,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub notify_limit_reached: bool,
    pub notify_low: bool,
    pub low_level: u8,
    pub notify_full_no_limit: bool,
    pub notify_app_drain: bool,
    pub notify_sleep_drain: bool,
    pub calibration: Calibration,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            notify_limit_reached: true,
            notify_low: true,
            low_level: 20,
            notify_full_no_limit: true,
            notify_app_drain: true,
            notify_sleep_drain: true,
            calibration: Calibration::Off,
        }
    }
}

fn path() -> PathBuf {
    gtk::glib::user_config_dir().join("battery-health-center").join("settings.ini")
}

impl Settings {
    pub fn load() -> Self {
        let text = fs::read_to_string(path()).unwrap_or_default();
        let map: HashMap<&str, &str> =
            text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
        let d = Settings::default();
        let flag = |k: &str, default: bool| map.get(k).map_or(default, |v| *v == "true");
        Settings {
            notify_limit_reached: flag("notify_limit_reached", d.notify_limit_reached),
            notify_low: flag("notify_low", d.notify_low),
            low_level: map.get("low_level").and_then(|v| v.parse().ok()).filter(|v| (5..=50).contains(v)).unwrap_or(d.low_level),
            notify_full_no_limit: flag("notify_full_no_limit", d.notify_full_no_limit),
            notify_app_drain: flag("notify_app_drain", d.notify_app_drain),
            notify_sleep_drain: flag("notify_sleep_drain", d.notify_sleep_drain),
            calibration: map.get("calibration").map_or(Calibration::Off, |v| Calibration::parse(v)),
        }
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let text = format!(
            "notify_limit_reached={}\nnotify_low={}\nlow_level={}\nnotify_full_no_limit={}\n\
             notify_app_drain={}\nnotify_sleep_drain={}\ncalibration={}\n",
            self.notify_limit_reached,
            self.notify_low,
            self.low_level,
            self.notify_full_no_limit,
            self.notify_app_drain,
            self.notify_sleep_drain,
            self.calibration.as_str()
        );
        let _ = fs::write(p, text);
    }
}
