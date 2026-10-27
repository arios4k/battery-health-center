use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::widgets::{value_row, Gauge};
use crate::battery::{format_hours, Battery, Condition, Status};
use crate::controller::{Controller, State, HIGH_SLEEP_DRAIN};
use crate::history::now_unix;
use crate::i18n::{tr, trf};

/// Chargers below this can't keep up with a busy laptop.
const WEAK_CHARGER_W: f64 = 45.0;

#[derive(Clone, Copy, PartialEq)]
enum Action {
    OpenCharging,
    OpenHealth,
    OpenUsage,
    UseBalanced,
}

#[derive(PartialEq)]
struct Recommendation {
    icon: &'static str,
    title: String,
    subtitle: String,
    action: Option<(&'static str, Action)>,
}

pub struct OverviewPage {
    pub root: adw::PreferencesPage,
    /// Gauge and status text; vertical on narrow windows.
    pub hero: gtk::Box,
    pub hero_text: gtk::Box,
    gauge: Gauge,
    headline: gtk::Label,
    detail: gtk::Label,
    power: gtk::Label,
    health: gtk::Label,
    health_bar: gtk::LevelBar,
    condition: gtk::Label,
    cycles: gtk::Label,
    power_group: adw::PreferencesGroup,
    charger_row: adw::ActionRow,
    unplugged_row: adw::ActionRow,
    unplugged: gtk::Label,
    used_row: adw::ActionRow,
    used: gtk::Label,
    avg_row: adw::ActionRow,
    avg: gtk::Label,
    devices_group: adw::PreferencesGroup,
    devices_list: gtk::ListBox,
    last_devices: RefCell<Vec<crate::system::DeviceBattery>>,
    recs_list: gtk::ListBox,
    last_recs: RefCell<Vec<Recommendation>>,
    stack: adw::ViewStack,
    ctrl: Weak<Controller>,
}

impl OverviewPage {
    pub fn new(ctrl: &Rc<Controller>, stack: &adw::ViewStack) -> Rc<Self> {
        let root = adw::PreferencesPage::new();

        // Hero: ring gauge with status text beside it.
        let hero_group = adw::PreferencesGroup::new();
        let gauge = Gauge::new();
        // A minimum width keeps the wrapping labels from shrinking to their
        // longest word; narrow windows stack the text under the gauge instead
        // (see the breakpoint in ui::build_window).
        let label = |classes: &[&str], chars: i32| {
            gtk::Label::builder().css_classes(classes).xalign(0.0).wrap(true).width_chars(chars).max_width_chars(28).build()
        };
        let headline = label(&["title-2"], 14);
        let detail = label(&["dim-label"], 20);
        let power = label(&["dim-label", "numeric"], 20);
        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();
        text.append(&headline);
        text.append(&detail);
        text.append(&power);
        let hero = gtk::Box::builder().spacing(24).halign(gtk::Align::Center).margin_bottom(12).build();
        let text_ref = text.clone();
        hero.append(&gauge.root);
        hero.append(&text);
        hero_group.add(&hero);
        root.add(&hero_group);

        let health_group = adw::PreferencesGroup::builder().title(tr("Battery Health")).build();
        let (health_row, health) = value_row(&health_group, tr("Maximum Capacity"));
        let health_bar = gtk::LevelBar::builder()
            .min_value(0.0)
            .max_value(100.0)
            .width_request(120)
            .valign(gtk::Align::Center)
            .build();
        health_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_LOW, 60.0);
        health_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_HIGH, 80.0);
        health_row.add_suffix(&health_bar);
        let (_, condition) = value_row(&health_group, tr("Condition"));
        let (_, cycles) = value_row(&health_group, tr("Cycle Count"));
        root.add(&health_group);

        // Charger while plugged in; battery use since unplugging otherwise.
        let power_group = adw::PreferencesGroup::builder().title(tr("Since Last Charge")).build();
        let charger_row = adw::ActionRow::builder().title(tr("Charger")).build();
        charger_row.add_prefix(&gtk::Image::from_icon_name("ac-adapter-symbolic"));
        power_group.add(&charger_row);
        let (unplugged_row, unplugged) = value_row(&power_group, tr("Unplugged"));
        let (used_row, used) = value_row(&power_group, tr("Battery Used"));
        let (avg_row, avg) = value_row(&power_group, tr("Average Draw"));
        root.add(&power_group);

        let devices_group = adw::PreferencesGroup::builder().title(tr("Other Devices")).visible(false).build();
        let devices_list = gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).build();
        devices_group.add(&devices_list);
        root.add(&devices_group);

        let recs_group = adw::PreferencesGroup::builder().title(tr("Recommendations")).build();
        let recs_list = gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).build();
        recs_group.add(&recs_list);
        root.add(&recs_group);

        Rc::new(OverviewPage {
            root,
            hero,
            hero_text: text_ref,
            gauge,
            headline,
            detail,
            power,
            health,
            health_bar,
            condition,
            cycles,
            power_group,
            charger_row,
            unplugged_row,
            unplugged,
            used_row,
            used,
            avg_row,
            avg,
            devices_group,
            devices_list,
            last_devices: RefCell::new(Vec::new()),
            recs_list,
            last_recs: RefCell::new(Vec::new()),
            stack: stack.clone(),
            ctrl: Rc::downgrade(ctrl),
        })
    }

    pub fn update(self: &Rc<Self>, st: &State) {
        let Some(b) = &st.battery else { return };
        let charging = b.status == Status::Charging;
        self.gauge.set(b.percent, charging, b.status.short_label());

        let (headline, detail) = match (b.status, b.hours_remaining()) {
            (Status::Charging, _) if st.full_charge => (
                b.hours_remaining().map_or(tr("Charging to 100%").into(), |h| trf("{0} until 100%", &[("0", format_hours(h).to_string())])),
                tr("One-time full charge; your limit comes back afterwards").into(),
            ),
            (Status::Discharging, Some(h)) => (trf("{0} remaining", &[("0", format_hours(h).to_string())]), tr("Estimated at the current usage").to_string()),
            (Status::Charging, Some(h)) => {
                let target = b.end_threshold.unwrap_or(100).min(100);
                (trf("{0} until {target}%", &[("0", format_hours(h).to_string()), ("target", target.to_string())]), tr("Charging").to_string())
            }
            (Status::Discharging, None) => (tr("On Battery").into(), tr("Estimating time remaining…").into()),
            (Status::NotCharging, _) if b.end_threshold.is_some_and(|e| e < 100) => (
                tr("Charging On Hold").into(),
                trf("Charge limit of {0}% protects your battery", &[("0", b.end_threshold.unwrap_or(100).to_string())]),
            ),
            (s, _) => (s.label().to_string(), if b.ac_online { tr("Connected to power").into() } else { String::new() }),
        };
        self.headline.set_label(&headline);
        self.detail.set_label(&detail);
        self.detail.set_visible(!detail.is_empty());
        match b.power_w.filter(|p| *p > 0.05) {
            Some(p) if b.status == Status::Discharging => self.power.set_label(&trf("Using {p} W", &[("p", format!("{p:.1}"))])),
            Some(p) => self.power.set_label(&trf("Charging at {p} W", &[("p", format!("{p:.1}"))])),
            None => self.power.set_label(""),
        }
        self.power.set_visible(!self.power.label().is_empty());

        match b.health_percent() {
            Some(h) => {
                self.health.set_label(&format!("{h:.0}%"));
                self.health_bar.set_value(h);
            }
            None => self.health.set_label(tr("Unknown")),
        }
        self.health_bar.set_visible(b.health_percent().is_some());
        self.condition.set_label(b.condition().map(Condition::label).unwrap_or(tr("Unknown")));
        self.cycles.set_label(&b.cycle_count.map(|c| c.to_string()).unwrap_or_else(|| tr("Not reported").into()));

        self.update_power(b, st);
        self.update_devices(st);
        self.update_recommendations(b, st);
    }

    fn update_power(&self, b: &Battery, st: &State) {
        let since = st.history.since_last_charge().filter(|_| b.is_on_battery());
        let plugged = b.ac_online && st.charger.is_some();
        self.power_group.set_title(if plugged { tr("Power Source") } else { tr("Since Last Charge") });
        self.power_group.set_visible(plugged || since.is_some());
        self.charger_row.set_visible(plugged);
        if let Some(c) = st.charger.as_ref() {
            let title = match c.watts() {
                Some(w) => format!("{} · {w:.0} W", c.kind),
                None => c.kind.clone(),
            };
            self.charger_row.set_title(&title);
            let subtitle = match (c.volts, c.amps) {
                (Some(v), Some(a)) => format!("{v:.0} V × {a:.2} A"),
                _ => tr("This charger doesn't report its power rating").into(),
            };
            self.charger_row.set_subtitle(&subtitle);
        }
        for row in [&self.unplugged_row, &self.used_row, &self.avg_row] {
            row.set_visible(since.is_some());
        }
        if let Some(s) = since {
            let ago = format_hours((now_unix() - s.unplugged_ts).max(60) as f64 / 3600.0);
            // If the app wasn't running when the charger came out, all it knows is
            // when it last saw the laptop plugged in.
            self.unplugged_row.set_title(if s.exact { tr("Unplugged") } else { tr("Last Seen Plugged In") });
            self.unplugged.set_label(&trf("{ago} ago, at {0}%", &[("ago", ago.to_string()), ("0", format!("{:.0}", s.start_percent))]));
            self.used.set_label(&format!("{:.0}%", s.used_percent));
            self.avg_row.set_visible(s.avg_w.is_some());
            self.avg.set_label(&s.avg_w.map(|w| format!("{w:.1} W")).unwrap_or_default());
        }
    }

    fn update_devices(&self, st: &State) {
        self.devices_group.set_visible(!st.devices.is_empty());
        if *self.last_devices.borrow() == st.devices {
            return;
        }
        while let Some(child) = self.devices_list.first_child() {
            self.devices_list.remove(&child);
        }
        for d in &st.devices {
            let row = adw::ActionRow::builder().title(&d.name).use_markup(false).build();
            row.add_prefix(&gtk::Image::from_icon_name(d.icon));
            let value = d.percent.map(|p| format!("{p:.0}%")).or(d.level.map(String::from)).unwrap_or_else(|| tr("Unknown").into());
            row.add_suffix(&gtk::Label::builder().label(value).css_classes(["dim-label", "numeric"]).build());
            if let Some(p) = d.percent {
                let bar = gtk::LevelBar::builder().max_value(100.0).value(p).width_request(100).valign(gtk::Align::Center).build();
                row.add_suffix(&bar);
            }
            self.devices_list.append(&row);
        }
        *self.last_devices.borrow_mut() = st.devices.clone();
    }

    fn recommendations(&self, b: &Battery, st: &State) -> Vec<Recommendation> {
        let mut recs = Vec::new();
        if let Some(h) = b.health_percent().filter(|h| *h < 80.0) {
            recs.push(Recommendation {
                icon: "dialog-warning-symbolic",
                title: trf("Battery capacity is down to {h}%", &[("h", format!("{h:.0}"))]),
                subtitle: tr("Your battery holds noticeably less charge than when it was new. Calibrating can correct a wrong reading; if it stays low, consider a replacement.").into(),
                action: Some((tr("Calibrate"), Action::OpenHealth)),
            });
        }
        if b.supports_end_threshold() && b.end_threshold.unwrap_or(100) >= 100 && b.ac_online {
            recs.push(Recommendation {
                icon: "battery-level-80-charging-symbolic",
                title: tr("Limit charging to 80%").into(),
                subtitle: tr("Your laptop is plugged in. Keeping the battery below full charge slows down wear.").into(),
                action: Some((tr("Set Up"), Action::OpenCharging)),
            });
        }
        if b.ac_online {
            if let Some(w) = st.charger.as_ref().and_then(|c| c.watts()) {
                if b.status == Status::Discharging {
                    recs.push(Recommendation {
                        icon: "ac-adapter-symbolic",
                        title: tr("Draining while plugged in").into(),
                        subtitle: trf("The {w} W charger can't keep up with your laptop's power use. Use a more powerful charger.", &[("w", format!("{w:.0}"))]),
                        action: None,
                    });
                } else if w < WEAK_CHARGER_W {
                    recs.push(Recommendation {
                        icon: "ac-adapter-symbolic",
                        title: trf("Low-power charger ({w} W)", &[("w", format!("{w:.0}"))]),
                        subtitle: tr("Charging may be slow, especially while you're using the laptop.").into(),
                        action: None,
                    });
                }
            }
        }
        if let Some(sleep) = st.history.sleep_sessions().first().filter(|s| s.percent_per_hour() >= HIGH_SLEEP_DRAIN) {
            recs.push(Recommendation {
                icon: "weather-clear-night-symbolic",
                title: trf("High drain during sleep ({0}% per hour)", &[("0", format!("{:.1}", sleep.percent_per_hour()))]),
                subtitle: tr("Your laptop used a lot of battery while asleep. See the sleep details for tips.").into(),
                action: Some((tr("Details"), Action::OpenUsage)),
            });
        }
        if b.is_on_battery() {
            if let Some(app) = st.apps_now.iter().find(|a| !a.system && a.impact >= 40.0) {
                recs.push(Recommendation {
                    icon: "power-profile-balanced-symbolic",
                    title: trf("{0} is using significant energy", &[("0", app.name.to_string())]),
                    subtitle: tr("Quitting apps you are not using makes your battery last longer.").into(),
                    action: Some((tr("Details"), Action::OpenUsage)),
                });
            }
            let ctrl = self.ctrl.upgrade();
            let profile = ctrl.as_ref().and_then(|c| c.profiles.as_ref()).and_then(|p| p.active());
            if profile.as_deref() == Some("performance") {
                recs.push(Recommendation {
                    icon: "power-profile-performance-symbolic",
                    title: tr("Performance mode is on").into(),
                    subtitle: tr("Performance mode drains the battery faster. Balanced is usually enough on battery.").into(),
                    action: Some((tr("Use Balanced"), Action::UseBalanced)),
                });
            }
            if b.percent <= 20.0 {
                recs.push(Recommendation {
                    icon: "battery-level-20-symbolic",
                    title: tr("Battery is low").into(),
                    subtitle: tr("Connect to power soon. Regularly running the battery flat wears it faster.").into(),
                    action: None,
                });
            }
        }
        if recs.is_empty() {
            recs.push(Recommendation {
                icon: "emblem-ok-symbolic",
                title: tr("Everything looks good").into(),
                subtitle: tr("No recommendations right now.").into(),
                action: None,
            });
        }
        recs
    }

    fn update_recommendations(self: &Rc<Self>, b: &Battery, st: &State) {
        let recs = self.recommendations(b, st);
        // Only rebuild when something changed, so buttons stay clickable.
        if *self.last_recs.borrow() == recs {
            return;
        }
        while let Some(child) = self.recs_list.first_child() {
            self.recs_list.remove(&child);
        }
        for rec in &recs {
            let row = adw::ActionRow::builder()
                .title(&rec.title)
                .subtitle(&rec.subtitle)
                .use_markup(false)
                .build();
            row.add_prefix(&gtk::Image::from_icon_name(rec.icon));
            if let Some((label, action)) = rec.action {
                let button = gtk::Button::builder().label(label).valign(gtk::Align::Center).build();
                let this = Rc::downgrade(self);
                button.connect_clicked(move |_| {
                    if let Some(this) = this.upgrade() {
                        this.run(action);
                    }
                });
                row.add_suffix(&button);
            }
            self.recs_list.append(&row);
        }
        *self.last_recs.borrow_mut() = recs;
    }

    fn run(&self, action: Action) {
        match action {
            Action::OpenCharging => self.stack.set_visible_child_name("charging"),
            Action::OpenHealth => self.stack.set_visible_child_name("health"),
            Action::OpenUsage => self.stack.set_visible_child_name("usage"),
            Action::UseBalanced => {
                if let Some(ctrl) = self.ctrl.upgrade() {
                    if let Some(p) = &ctrl.profiles {
                        let _ = p.set_active("balanced");
                    }
                    ctrl.refresh();
                }
            }
        }
    }
}
