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
    full_group: adw::PreferencesGroup,
    full_row: adw::ActionRow,
    full_button: gtk::Button,
    mode_group: adw::PreferencesGroup,
    mode: adw::ComboRow,
    mode_ids: RefCell<Vec<String>>,
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

        let full_group = adw::PreferencesGroup::new();
        let full_row = adw::ActionRow::builder().title("Charge to 100% Now").build();
        full_row.add_prefix(&gtk::Image::from_icon_name("battery-level-100-charged-symbolic"));
        let full_button = gtk::Button::builder().label("Charge Now").valign(gtk::Align::Center).build();
        full_row.add_suffix(&full_button);
        full_group.add(&full_row);
        root.add(&full_group);

        let mode_group = adw::PreferencesGroup::builder()
            .title("Charging Mode")
            .description("Temporarily pause charging, or run from the battery while still plugged in.")
            .visible(false)
            .build();
        let mode = adw::ComboRow::builder().title("Mode").build();
        mode_group.add(&mode);
        root.add(&mode_group);

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
            full_group,
            full_row,
            full_button,
            mode_group,
            mode,
            mode_ids: RefCell::new(Vec::new()),
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
        self.full_button.connect_clicked(move |_| {
            let Some(this) = this.upgrade() else { return };
            let Some(ctrl) = this.ctrl.upgrade() else { return };
            if ctrl.state().full_charge {
                ctrl.finish_full_charge();
                this.toast("Back to your charge limit");
            } else {
                ctrl.start_full_charge();
                this.toast("Charging to 100% once; your limit comes back when full or unplugged");
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
                this.run_helper(vec![vec!["set-behaviour".into(), this.battery_name(), id.clone()]], format!("Charging mode set to {}", behaviour_label(&id)));
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
        let batteries = ctrl.state().batteries.clone();
        let Some(b) = batteries.first() else { return };
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
        // The same limit applies to every battery (some laptops have two).
        let requests = batteries
            .iter()
            .map(|b| vec!["set-thresholds".into(), b.name.clone(), start.clone(), end.to_string(), persist.into()])
            .collect();
        self.run_helper(requests, msg);
    }

    /// Runs helper requests in order, stopping at the first error.
    fn run_helper(self: &Rc<Self>, requests: Vec<Vec<String>>, success: String) {
        if self.busy.replace(true) {
            return;
        }
        self.apply.set_sensitive(false);
        let this = self.clone();
        glib::spawn_future_local(async move {
            let mut result = Ok(());
            for args in requests {
                result = system::run_helper(args).await;
                if result.is_err() {
                    break;
                }
            }
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
        // One-time full charge: only useful with a limit set, and while plugged in.
        self.full_group.set_visible(supported && (end < 100 || st.full_charge));
        if st.full_charge {
            self.full_row.set_title("Charging to 100%");
            self.full_row.set_subtitle("Your limit comes back when the battery is full or you unplug.");
            self.full_button.set_label("Cancel");
            self.full_button.set_sensitive(true);
            self.full_button.remove_css_class("suggested-action");
        } else {
            self.full_row.set_title("Charge to 100% Now");
            self.full_row.set_subtitle(if b.ac_online {
                "Charge fully once, for a trip or a long day. Your limit comes back when full or unplugged."
            } else {
                "Connect the charger to charge fully once."
            });
            self.full_button.set_label("Charge Now");
            self.full_button.set_sensitive(b.ac_online);
        }
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
            // Default to keeping the limit: a limit that is active but not yet
            // saved (e.g. after reinstalling) should be saved on the next Apply.
            self.persist.set_active(true);
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

        self.updating.set(false);
    }
}
