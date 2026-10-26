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
    /// Switch the power mode by itself when the charger is connected or removed.
    pub auto_profile: bool,
    /// Power mode ids (power-saver, balanced, performance) to use while plugged in
    /// and on battery.
    pub profile_ac: String,
    pub profile_battery: String,
    /// On battery, switch to Power Saver once the charge falls to this level.
    pub low_profile: bool,
    pub low_profile_level: u8,
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
            auto_profile: false,
            profile_ac: "balanced".into(),
            profile_battery: "balanced".into(),
            low_profile: true,
            low_profile_level: 20,
        }
    }
}

fn path() -> PathBuf {
    gtk::glib::user_config_dir().join("battery-health-center").join("settings.ini")
}

/// Power mode ids this app knows; anything else in the file is ignored.
fn valid_profile(p: &str) -> bool {
    matches!(p, "power-saver" | "balanced" | "performance")
}

impl Settings {
    pub fn load() -> Self {
        Self::parse(&fs::read_to_string(path()).unwrap_or_default())
    }

    fn parse(text: &str) -> Self {
        let map: HashMap<&str, &str> =
            text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
        let d = Settings::default();
        let flag = |k: &str, default: bool| map.get(k).map_or(default, |v| *v == "true");
        let level = |k: &str, default: u8, range: std::ops::RangeInclusive<u8>| {
            map.get(k).and_then(|v| v.parse().ok()).filter(|v| range.contains(v)).unwrap_or(default)
        };
        let profile = |k: &str, default: &str| {
            map.get(k).filter(|v| valid_profile(v)).map_or_else(|| default.to_string(), |v| v.to_string())
        };
        Settings {
            notify_limit_reached: flag("notify_limit_reached", d.notify_limit_reached),
            notify_low: flag("notify_low", d.notify_low),
            low_level: level("low_level", d.low_level, 5..=50),
            notify_full_no_limit: flag("notify_full_no_limit", d.notify_full_no_limit),
            notify_app_drain: flag("notify_app_drain", d.notify_app_drain),
            notify_sleep_drain: flag("notify_sleep_drain", d.notify_sleep_drain),
            calibration: map.get("calibration").map_or(Calibration::Off, |v| Calibration::parse(v)),
            auto_profile: flag("auto_profile", d.auto_profile),
            profile_ac: profile("profile_ac", &d.profile_ac),
            profile_battery: profile("profile_battery", &d.profile_battery),
            low_profile: flag("low_profile", d.low_profile),
            low_profile_level: level("low_profile_level", d.low_profile_level, 5..=60),
        }
    }

    fn to_text(&self) -> String {
        format!(
            "notify_limit_reached={}\nnotify_low={}\nlow_level={}\nnotify_full_no_limit={}\n\
             notify_app_drain={}\nnotify_sleep_drain={}\ncalibration={}\n\
             auto_profile={}\nprofile_ac={}\nprofile_battery={}\nlow_profile={}\nlow_profile_level={}\n",
            self.notify_limit_reached,
            self.notify_low,
            self.low_level,
            self.notify_full_no_limit,
            self.notify_app_drain,
            self.notify_sleep_drain,
            self.calibration.as_str(),
            self.auto_profile,
            self.profile_ac,
            self.profile_battery,
            self.low_profile,
            self.low_profile_level
        )
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(p, self.to_text());
    }

    /// The power mode to use right now under "automatic power mode", or None
    /// when it's off or no usable mode exists. A mode the system doesn't offer
    /// (e.g. performance on many laptops) falls back to balanced.
    pub fn desired_profile(&self, plugged_in: bool, percent: f64, available: &[String]) -> Option<String> {
        if !self.auto_profile {
            return None;
        }
        let has = |p: &str| available.iter().any(|a| a == p);
        let mut want = if plugged_in {
            self.profile_ac.as_str()
        } else if self.low_profile && percent <= self.low_profile_level as f64 {
            "power-saver"
        } else {
            self.profile_battery.as_str()
        };
        if !has(want) {
            want = "balanced";
        }
        has(want).then(|| want.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<String> {
        ["power-saver", "balanced", "performance"].map(String::from).to_vec()
    }

    fn auto() -> Settings {
        Settings {
            auto_profile: true,
            profile_ac: "performance".into(),
            profile_battery: "balanced".into(),
            low_profile: true,
            low_profile_level: 30,
            ..Settings::default()
        }
    }

    #[test]
    fn text_roundtrip() {
        let s = Settings { low_level: 35, auto_profile: true, profile_ac: "performance".into(), ..Settings::default() };
        assert_eq!(Settings::parse(&s.to_text()), s);
    }

    #[test]
    fn bad_values_fall_back_to_defaults() {
        let s = Settings::parse("profile_ac=turbo\nlow_profile_level=3\nlow_level=999\nauto_profile=maybe\n");
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn disabled_does_nothing() {
        assert_eq!(Settings::default().desired_profile(true, 50.0, &all()), None);
    }

    #[test]
    fn follows_the_charger_and_the_low_level() {
        let s = auto();
        assert_eq!(s.desired_profile(true, 50.0, &all()).as_deref(), Some("performance"));
        assert_eq!(s.desired_profile(false, 80.0, &all()).as_deref(), Some("balanced"));
        assert_eq!(s.desired_profile(false, 30.0, &all()).as_deref(), Some("power-saver"));
        assert_eq!(s.desired_profile(false, 12.0, &all()).as_deref(), Some("power-saver"));
        // Plugged in wins over a low charge.
        assert_eq!(s.desired_profile(true, 5.0, &all()).as_deref(), Some("performance"));
    }

    #[test]
    fn low_switch_can_be_off() {
        let s = Settings { low_profile: false, ..auto() };
        assert_eq!(s.desired_profile(false, 5.0, &all()).as_deref(), Some("balanced"));
    }

    #[test]
    fn missing_modes_fall_back() {
        let two: Vec<String> = ["power-saver", "balanced"].map(String::from).to_vec();
        assert_eq!(auto().desired_profile(true, 50.0, &two).as_deref(), Some("balanced"));
        assert_eq!(auto().desired_profile(true, 50.0, &[]), None);
    }
}
