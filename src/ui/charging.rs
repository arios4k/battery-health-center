use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;

use crate::controller::{Controller, State};
use crate::system;

fn behaviour_label(id: &str) -> &str {
    match id {
        "auto" => "Normal",
        "inhibit-charge" => "Pause Charging",
        "force-discharge" => "Run on Battery",
        other => other,
    }
}

pub struct ChargingPage {
    pub root: adw::PreferencesPage,
    limit_group: adw::PreferencesGroup,
    unsupported: adw::PreferencesGroup,
    status_row: adw::ActionRow,
    enable: adw::SwitchRow,
    end: adw::SpinRow,
    start: adw::SpinRow,
    persist: adw::SwitchRow,
    apply: gtk::Button,
    mode_group: adw::PreferencesGroup,
    mode: adw::ComboRow,
    mode_ids: RefCell<Vec<String>>,
    profile_group: adw::PreferencesGroup,
    profile: adw::ComboRow,
    profile_ids: RefCell<Vec<String>>,
    initialized: Cell<bool>,
    /// Set while the UI is updated programmatically, to ignore change signals.
    updating: Cell<bool>,
    busy: Cell<bool>,
    ctrl: Weak<Controller>,
    toasts: adw::ToastOverlay,
}

impl ChargingPage {
    pub fn new(ctrl: &Rc<Controller>, toasts: &adw::ToastOverlay) -> Rc<Self> {
        let root = adw::PreferencesPage::new();

        let limit_group = adw::PreferencesGroup::builder()
            .title("Charge Limit")
            .description(
                "Lithium batteries wear faster when kept at full charge. If your laptop is plugged in most \
                 of the time, limiting the charge to about 80% can noticeably extend the battery's lifespan.",
            )
            .build();
        let status_row = adw::ActionRow::builder().title("Current Setting").build();
        status_row.add_prefix(&gtk::Image::from_icon_name("battery-level-80-charging-symbolic"));
        let enable = adw::SwitchRow::builder().title("Limit Maximum Charge").build();
        let end = adw::SpinRow::builder()
            .title("Stop Charging At")
            .subtitle("Percent")
            .adjustment(&gtk::Adjustment::new(80.0, 50.0, 100.0, 1.0, 5.0, 0.0))
            .build();
        let start = adw::SpinRow::builder()
            .title("Resume Charging Below")
            .subtitle("Avoids topping up after every small drop")
            .adjustment(&gtk::Adjustment::new(75.0, 0.0, 99.0, 1.0, 5.0, 0.0))
            .build();
        let persist = adw::SwitchRow::builder()
            .title("Keep After Restart")
            .subtitle("Re-apply the limit at startup and after waking from sleep")
            .active(true)
            .build();
        let apply = gtk::Button::builder()
            .label("Apply")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk::Align::Center)
            .margin_top(12)
            .build();
        limit_group.add(&status_row);
        limit_group.add(&enable);
        limit_group.add(&end);
        limit_group.add(&start);
        limit_group.add(&persist);
        limit_group.add(&apply);
        root.add(&limit_group);

        let unsupported = adw::PreferencesGroup::builder().title("Charge Limit").visible(false).build();
        let row = adw::ActionRow::builder()
            .title("Not supported on this laptop")
            .subtitle("The battery driver does not expose charge thresholds (charge_control_end_threshold).")
            .build();
        row.add_prefix(&gtk::Image::from_icon_name("dialog-information-symbolic"));
        unsupported.add(&row);
        root.add(&unsupported);

        let mode_group = adw::PreferencesGroup::builder()
            .title("Charging Mode")
            .description("Temporarily pause charging, or run from the battery while still plugged in.")
            .visible(false)
            .build();
        let mode = adw::ComboRow::builder().title("Mode").build();
        mode_group.add(&mode);
        root.add(&mode_group);

        let profile_group = adw::PreferencesGroup::builder()
            .title("Power Mode")
            .description("Power Saver reduces performance to extend battery life.")
            .visible(false)
            .build();
        let profile = adw::ComboRow::builder().title("Power Mode").build();
        profile_group.add(&profile);
        root.add(&profile_group);

        let page = Rc::new(ChargingPage {
            root,
            limit_group,
            unsupported,
            status_row,
            enable,
            end,
            start,
            persist,
            apply,
            mode_group,
            mode,
            mode_ids: RefCell::new(Vec::new()),
            profile_group,
            profile,
            profile_ids: RefCell::new(Vec::new()),
            initialized: Cell::new(false),
            updating: Cell::new(false),
            busy: Cell::new(false),
            ctrl: Rc::downgrade(ctrl),
            toasts: toasts.clone(),
        });
        page.connect_signals();
        page
    }

    fn connect_signals(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        self.enable.connect_active_notify(move |_| {
            if let Some(this) = this.upgrade() {
                this.sync_sensitivity();
            }
        });
        let this = Rc::downgrade(self);
        self.end.connect_value_notify(move |end| {
            if let Some(this) = this.upgrade() {
                // Keep the resume point below the stop point.
                if this.start.value() >= end.value() {
                    this.start.set_value(end.value() - 5.0);
                }
            }
        });
        let this = Rc::downgrade(self);
        self.apply.connect_clicked(move |_| {
            if let Some(this) = this.upgrade() {
                this.apply_limit();
            }
        });
        let this = Rc::downgrade(self);
        self.mode.connect_selected_notify(move |row| {
            let Some(this) = this.upgrade() else { return };
            if this.updating.get() {
                return;
            }
            let id = this.mode_ids.borrow().get(row.selected() as usize).cloned();
            if let Some(id) = id {
                this.run_helper(vec!["set-behaviour".into(), this.battery_name(), id.clone()], format!("Charging mode set to {}", behaviour_label(&id)));
            }
        });
        let this = Rc::downgrade(self);
        self.profile.connect_selected_notify(move |row| {
            let Some(this) = this.upgrade() else { return };
            if this.updating.get() {
                return;
            }
            let Some(ctrl) = this.ctrl.upgrade() else { return };
            let id = this.profile_ids.borrow().get(row.selected() as usize).cloned();
            if let (Some(profiles), Some(id)) = (&ctrl.profiles, id) {
                if let Err(e) = profiles.set_active(&id) {
                    this.toast(&format!("Could not change power mode: {}", e.message()));
                }
            }
        });
    }

    fn sync_sensitivity(&self) {
        let on = self.enable.is_active();
        self.end.set_sensitive(on);
        self.start.set_sensitive(on);
    }

    fn battery_name(&self) -> String {
        self.ctrl
            .upgrade()
            .and_then(|c| c.state().battery.as_ref().map(|b| b.name.clone()))
            .unwrap_or_default()
    }

    fn toast(&self, msg: &str) {
        self.toasts.add_toast(adw::Toast::new(msg));
    }

    fn apply_limit(self: &Rc<Self>) {
        let Some(ctrl) = self.ctrl.upgrade() else { return };
        let Some(b) = ctrl.state().battery.clone() else { return };
        let on = self.enable.is_active();
        let end = if on { self.end.value() as u8 } else { 100 };
        let start = match (b.supports_start_threshold(), on) {
            (false, _) => "-".to_string(),
            (true, true) => (self.start.value() as u8).min(end.saturating_sub(1)).to_string(),
            // Charge whenever below 100% (0 means "no start threshold").
            (true, false) => "0".to_string(),
        };
        let persist = if self.persist.is_active() && on { "--persist" } else { "--no-persist" };
        let msg = if on { format!("Charging will stop at {end}%") } else { "Charge limit removed".to_string() };
        self.run_helper(vec!["set-thresholds".into(), b.name, start, end.to_string(), persist.into()], msg);
    }

    fn run_helper(self: &Rc<Self>, args: Vec<String>, success: String) {
        if self.busy.replace(true) {
            return;
        }
        self.apply.set_sensitive(false);
        let this = self.clone();
        glib::spawn_future_local(async move {
            let result = system::run_helper(args).await;
            this.busy.set(false);
            this.apply.set_sensitive(true);
            match result {
                Ok(()) => this.toast(&success),
                Err(e) => this.toast(&e),
            }
            if let Some(ctrl) = this.ctrl.upgrade() {
                ctrl.refresh();
            }
        });
    }

    pub fn update(&self, st: &State) {
        let Some(b) = &st.battery else { return };
        self.updating.set(true);

        let supported = b.supports_end_threshold();
        self.limit_group.set_visible(supported);
        self.unsupported.set_visible(!supported);
        self.start.set_visible(b.supports_start_threshold());

        let end = b.end_threshold.unwrap_or(100);
        let status = match (end < 100, b.start_threshold.filter(|s| *s > 0)) {
            (true, Some(s)) => format!("Stops at {end}%, resumes below {s}%"),
            (true, None) => format!("Stops at {end}%"),
            (false, _) => "Charges to 100%".to_string(),
        };
        self.status_row.set_subtitle(&status);

        // Load the current kernel values into the controls once; after that
        // the controls hold the user's pending edits.
        if !self.initialized.get() && supported {
            self.initialized.set(true);
            self.enable.set_active(end < 100);
            self.end.set_value(if end < 100 { end as f64 } else { 80.0 });
            let start = b.start_threshold.filter(|s| *s > 0 && *s < end).map(|s| s as f64);
            self.start.set_value(start.unwrap_or(self.end.value() - 5.0));
            self.persist.set_active(end >= 100 || system::limit_persisted(&b.name));
            self.sync_sensitivity();
        }

        // Charging mode (charge_behaviour)
        // The snap's interfaces don't allow writing charge_behaviour.
        let has_modes = b.charge_behaviours.len() > 1 && !system::in_snap();
        self.mode_group.set_visible(has_modes);
        if has_modes {
            if *self.mode_ids.borrow() != b.charge_behaviours {
                let labels: Vec<&str> = b.charge_behaviours.iter().map(|s| behaviour_label(s)).collect();
                self.mode.set_model(Some(&gtk::StringList::new(&labels)));
                *self.mode_ids.borrow_mut() = b.charge_behaviours.clone();
            }
            if let Some(i) = b.charge_behaviours.iter().position(|m| Some(m) == b.charge_behaviour.as_ref()) {
                if !self.busy.get() {
                    self.mode.set_selected(i as u32);
                }
            }
        }

        // Power profile
        if let Some(ctrl) = self.ctrl.upgrade() {
            if let Some(profiles) = &ctrl.profiles {
                let ids = profiles.available();
                self.profile_group.set_visible(!ids.is_empty());
                if *self.profile_ids.borrow() != ids {
                    let labels: Vec<&str> = ids.iter().map(|s| system::profile_label(s)).collect();
                    self.profile.set_model(Some(&gtk::StringList::new(&labels)));
                    *self.profile_ids.borrow_mut() = ids.clone();
                }
                if let Some(i) = profiles.active().and_then(|a| ids.iter().position(|p| *p == a)) {
                    self.profile.set_selected(i as u32);
                }
            }
        }

        self.updating.set(false);
    }
}
