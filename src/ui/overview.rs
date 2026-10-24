use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::widgets::{value_row, Gauge};
use crate::battery::{format_hours, Battery, Condition, Status};
use crate::controller::{Controller, State};

#[derive(Clone, Copy, PartialEq)]
enum Action {
    OpenCharging,
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
    gauge: Gauge,
    headline: gtk::Label,
    detail: gtk::Label,
    power: gtk::Label,
    health: gtk::Label,
    health_bar: gtk::LevelBar,
    condition: gtk::Label,
    cycles: gtk::Label,
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
        let headline = gtk::Label::builder().css_classes(["title-2"]).xalign(0.0).wrap(true).build();
        let detail = gtk::Label::builder().css_classes(["dim-label"]).xalign(0.0).wrap(true).build();
        let power = gtk::Label::builder().css_classes(["dim-label", "numeric"]).xalign(0.0).wrap(true).build();
        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();
        text.append(&headline);
        text.append(&detail);
        text.append(&power);
        let hero = gtk::Box::builder().spacing(24).halign(gtk::Align::Center).margin_bottom(12).build();
        hero.append(&gauge.root);
        hero.append(&text);
        hero_group.add(&hero);
        root.add(&hero_group);

        let health_group = adw::PreferencesGroup::builder().title("Battery Health").build();
        let (health_row, health) = value_row(&health_group, "Maximum Capacity");
        let health_bar = gtk::LevelBar::builder()
            .min_value(0.0)
            .max_value(100.0)
            .width_request(120)
            .valign(gtk::Align::Center)
            .build();
        health_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_LOW, 60.0);
        health_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_HIGH, 80.0);
        health_row.add_suffix(&health_bar);
        let (_, condition) = value_row(&health_group, "Condition");
        let (_, cycles) = value_row(&health_group, "Cycle Count");
        root.add(&health_group);

        let recs_group = adw::PreferencesGroup::builder().title("Recommendations").build();
        let recs_list = gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).build();
        recs_group.add(&recs_list);
        root.add(&recs_group);

        Rc::new(OverviewPage {
            root,
            gauge,
            headline,
            detail,
            power,
            health,
            health_bar,
            condition,
            cycles,
            recs_list,
            last_recs: RefCell::new(Vec::new()),
            stack: stack.clone(),
            ctrl: Rc::downgrade(ctrl),
        })
    }

    pub fn update(self: &Rc<Self>, st: &State) {
        let Some(b) = &st.battery else { return };
        let charging = b.status == Status::Charging;
        self.gauge.set(b.percent, charging, b.status.label());

        let (headline, detail) = match (b.status, b.hours_remaining()) {
            (Status::Discharging, Some(h)) => (format!("{} remaining", format_hours(h)), "Estimated at the current usage".to_string()),
            (Status::Charging, Some(h)) => {
                let target = b.end_threshold.unwrap_or(100).min(100);
                (format!("{} until {target}%", format_hours(h)), "Charging".to_string())
            }
            (Status::Discharging, None) => ("On Battery".into(), "Estimating time remaining…".into()),
            (Status::NotCharging, _) if b.end_threshold.is_some_and(|e| e < 100) => (
                "Charging On Hold".into(),
                format!("Charge limit of {}% protects your battery", b.end_threshold.unwrap_or(100)),
            ),
            (s, _) => (s.label().to_string(), if b.ac_online { "Connected to power".into() } else { String::new() }),
        };
        self.headline.set_label(&headline);
        self.detail.set_label(&detail);
        self.detail.set_visible(!detail.is_empty());
        match b.power_w.filter(|p| *p > 0.05) {
            Some(p) if b.status == Status::Discharging => self.power.set_label(&format!("Using {p:.1} W")),
            Some(p) => self.power.set_label(&format!("Charging at {p:.1} W")),
            None => self.power.set_label(""),
        }
        self.power.set_visible(!self.power.label().is_empty());

        match b.health_percent() {
            Some(h) => {
                self.health.set_label(&format!("{h:.0}%"));
                self.health_bar.set_value(h);
            }
            None => self.health.set_label("Unknown"),
        }
        self.health_bar.set_visible(b.health_percent().is_some());
        self.condition.set_label(b.condition().map(Condition::label).unwrap_or("Unknown"));
        self.cycles.set_label(&b.cycle_count.map(|c| c.to_string()).unwrap_or_else(|| "Not reported".into()));

        self.update_recommendations(b, st);
    }

    fn recommendations(&self, b: &Battery, st: &State) -> Vec<Recommendation> {
        let mut recs = Vec::new();
        if let Some(h) = b.health_percent().filter(|h| *h < 80.0) {
            recs.push(Recommendation {
                icon: "dialog-warning-symbolic",
                title: format!("Battery capacity is down to {h:.0}%"),
                subtitle: "Your battery holds noticeably less charge than when it was new. Consider a replacement if runtime is too short.".into(),
                action: None,
            });
        }
        if b.supports_end_threshold() && b.end_threshold.unwrap_or(100) >= 100 && b.ac_online {
            recs.push(Recommendation {
                icon: "battery-level-80-charging-symbolic",
                title: "Limit charging to 80%".into(),
                subtitle: "Your laptop is plugged in. Keeping the battery below full charge slows down wear.".into(),
                action: Some(("Set Up", Action::OpenCharging)),
            });
        }
        if b.is_on_battery() {
            if let Some(app) = st.apps_now.iter().find(|a| !a.system && a.impact >= 40.0) {
                recs.push(Recommendation {
                    icon: "utilities-system-monitor-symbolic",
                    title: format!("{} is using significant energy", app.name),
                    subtitle: "Quitting apps you are not using makes your battery last longer.".into(),
                    action: Some(("Details", Action::OpenUsage)),
                });
            }
            let ctrl = self.ctrl.upgrade();
            let profile = ctrl.as_ref().and_then(|c| c.profiles.as_ref()).and_then(|p| p.active());
            if profile.as_deref() == Some("performance") {
                recs.push(Recommendation {
                    icon: "power-profile-performance-symbolic",
                    title: "Performance mode is on".into(),
                    subtitle: "Performance mode drains the battery faster. Balanced is usually enough on battery.".into(),
                    action: Some(("Use Balanced", Action::UseBalanced)),
                });
            }
            if b.percent <= 20.0 {
                recs.push(Recommendation {
                    icon: "battery-level-20-symbolic",
                    title: "Battery is low".into(),
                    subtitle: "Connect to power soon. Regularly running the battery flat wears it faster.".into(),
                    action: None,
                });
            }
        }
        if recs.is_empty() {
            recs.push(Recommendation {
                icon: "emblem-ok-symbolic",
                title: "Everything looks good".into(),
                subtitle: "No recommendations right now.".into(),
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
