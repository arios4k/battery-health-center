use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;

use crate::controller::{Controller, State};
use crate::system;
use crate::i18n::{tr, trf};
use crate::schedule::{self, Learned, Rule, Schedule};

fn behaviour_label(id: &str) -> &str {
    match id {
        "auto" => tr("Normal"),
        "inhibit-charge" => tr("Pause Charging"),
        "force-discharge" => tr("Run on Battery"),
        other => other,
    }
}

/// Charge limit presets: where charging stops and where it resumes. A stop point
/// of 100 means no limit.
const PROFILES: [(u8, u8); 3] = [(60, 55), (80, 75), (100, 0)];
/// Index of the entry that stands for settings that match no preset.
const CUSTOM_PROFILE: usize = PROFILES.len();

fn profile_label(i: usize) -> &'static str {
    match i {
        0 => tr("Maximum Lifespan (60%)"),
        1 => tr("Balanced (80%)"),
        2 => tr("Full Capacity (100%)"),
        _ => tr("Custom"),
    }
}

fn profile_hint(i: usize) -> &'static str {
    match i {
        0 => tr("Stops at 60%. Best when you're plugged in most of the time."),
        1 => tr("Stops at 80%. A good everyday balance."),
        2 => tr("Charges to 100%. For the longest time away from the charger."),
        _ => tr("Your own settings below."),
    }
}

/// The entry that matches a battery's current stop point.
fn profile_for(end: u8) -> usize {
    PROFILES.iter().position(|(stop, _)| *stop == end).unwrap_or(CUSTOM_PROFILE)
}

fn weekday_name(i: u8) -> &'static str {
    match i {
        0 => tr("Mon"),
        1 => tr("Tue"),
        2 => tr("Wed"),
        3 => tr("Thu"),
        4 => tr("Fri"),
        5 => tr("Sat"),
        _ => tr("Sun"),
    }
}

/// "Weekdays", "Every day", "Mon, Wed" ...
fn days_label(days: u8) -> String {
    match days {
        schedule::EVERY_DAY => tr("Every day").to_string(),
        schedule::WEEKDAYS => tr("Weekdays").to_string(),
        schedule::WEEKEND => tr("Weekends").to_string(),
        _ => (0..7).filter(|i| days >> i & 1 == 1).map(weekday_name).collect::<Vec<_>>().join(", "),
    }
}

/// What has been learned so far, for the "Learned" row.
fn learned_text(l: &Learned) -> String {
    let line = |name: &str, time: Option<u16>, samples: usize| match time {
        Some(m) => trf("{days}: charged by about {time}", &[("days", name.to_string()), ("time", schedule::format_time(m))]),
        None => trf(
            "{days}: still learning ({n} of {min} unplugs)",
            &[("days", name.to_string()), ("n", samples.min(schedule::MIN_SAMPLES).to_string()), ("min", schedule::MIN_SAMPLES.to_string())],
        ),
    };
    format!(
        "{}\n{}",
        line(tr("Weekdays"), l.weekdays, l.weekday_samples),
        line(tr("Weekends"), l.weekend, l.weekend_samples)
    )
}

pub struct ChargingPage {
    pub root: adw::PreferencesPage,
    limit_group: adw::PreferencesGroup,
    unsupported: adw::PreferencesGroup,
    status_row: adw::ActionRow,
    profile: adw::ComboRow,
    enable: adw::SwitchRow,
    end: adw::SpinRow,
    start: adw::SpinRow,
    persist: adw::SwitchRow,
    apply: gtk::Button,
    schedule_group: adw::PreferencesGroup,
    optimized: adw::SwitchRow,
    learned_row: adw::ActionRow,
    lead: adw::SpinRow,
    add_row: adw::ActionRow,
    save_schedule: gtk::Button,
    rule_rows: RefCell<Vec<adw::ActionRow>>,
    /// The schedule as edited here, and as last saved.
    plan: RefCell<Schedule>,
    saved_plan: RefCell<Schedule>,
    full_group: adw::PreferencesGroup,
    full_row: adw::ActionRow,
    full_button: gtk::Button,
    mode_group: adw::PreferencesGroup,
    mode: adw::ComboRow,
    mode_ids: RefCell<Vec<String>>,
    initialized: Cell<bool>,
    schedule_loaded: Cell<bool>,
    /// The stop point the profile was last matched against.
    profile_end: Cell<Option<u8>>,
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
            .title(tr("Charge Limit"))
            .description(
                tr("Lithium batteries wear faster when kept at full charge. If your laptop is plugged in most of the time, limiting the charge to about 80% can noticeably extend the battery's lifespan."),
            )
            .build();
        let status_row = adw::ActionRow::builder().title(tr("Current Setting")).build();
        status_row.add_prefix(&gtk::Image::from_icon_name("battery-level-80-charging-symbolic"));
        let profile_labels: Vec<&str> = (0..=CUSTOM_PROFILE).map(profile_label).collect();
        let profile = adw::ComboRow::builder()
            .title(tr("Profile"))
            .subtitle(profile_hint(CUSTOM_PROFILE))
            .model(&gtk::StringList::new(&profile_labels))
            .selected(CUSTOM_PROFILE as u32)
            .build();
        let enable = adw::SwitchRow::builder().title(tr("Limit Maximum Charge")).build();
        let end = adw::SpinRow::builder()
            .title(tr("Stop Charging At"))
            .subtitle(tr("Percent"))
            .adjustment(&gtk::Adjustment::new(80.0, 50.0, 100.0, 1.0, 5.0, 0.0))
            .build();
        let start = adw::SpinRow::builder()
            .title(tr("Resume Charging Below"))
            .subtitle(tr("Avoids topping up after every small drop"))
            .adjustment(&gtk::Adjustment::new(75.0, 0.0, 99.0, 1.0, 5.0, 0.0))
            .build();
        let persist = adw::SwitchRow::builder()
            .title(tr("Keep After Restart"))
            .subtitle(tr("Re-apply the limit at startup and after waking from sleep"))
            .active(true)
            .build();
        let apply = gtk::Button::builder()
            .label(tr("Apply"))
            .css_classes(["suggested-action", "pill"])
            .halign(gtk::Align::Center)
            .margin_top(12)
            .build();
        limit_group.add(&status_row);
        limit_group.add(&profile);
        limit_group.add(&enable);
        limit_group.add(&end);
        limit_group.add(&start);
        limit_group.add(&persist);
        limit_group.add(&apply);
        root.add(&limit_group);

        let schedule_group = adw::PreferencesGroup::builder()
            .title(tr("Charging Schedule"))
            .description(tr("Charge to 100% just before you need the laptop, and keep your limit the rest of the time. This needs a saved charge limit: turn on Limit Maximum Charge and Keep After Restart, then Apply."))
            .build();
        let optimized = adw::SwitchRow::builder()
            .title(tr("Optimized Charging"))
            .subtitle(tr("Learns when you usually unplug and charges to 100% just before"))
            .build();
        let learned_row = adw::ActionRow::builder().title(tr("Learned So Far")).css_classes(["property"]).visible(false).build();
        learned_row.add_prefix(&gtk::Image::from_icon_name("preferences-system-time-symbolic"));
        let lead = adw::SpinRow::builder()
            .title(tr("Start Charging Before"))
            .subtitle(tr("Hours before the time, to have it full by then"))
            .adjustment(&gtk::Adjustment::new(2.0, schedule::MIN_LEAD_MIN as f64 / 60.0, schedule::MAX_LEAD_MIN as f64 / 60.0, 0.5, 1.0, 0.0))
            .digits(1)
            .build();
        let add_row = adw::ActionRow::builder().title(tr("Add a Time…")).activatable(true).build();
        add_row.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
        let save_schedule = gtk::Button::builder()
            .label(tr("Save Schedule"))
            .css_classes(["suggested-action", "pill"])
            .halign(gtk::Align::Center)
            .margin_top(12)
            .sensitive(false)
            .build();
        schedule_group.add(&optimized);
        schedule_group.add(&learned_row);
        schedule_group.add(&lead);
        schedule_group.add(&add_row);
        schedule_group.add(&save_schedule);
        root.add(&schedule_group);

        let unsupported = adw::PreferencesGroup::builder().title(tr("Charge Limit")).visible(false).build();
        let row = adw::ActionRow::builder()
            .title(tr("Not supported on this laptop"))
            .subtitle(tr("The battery driver does not expose charge thresholds (charge_control_end_threshold)."))
            .build();
        row.add_prefix(&gtk::Image::from_icon_name("dialog-information-symbolic"));
        unsupported.add(&row);
        root.add(&unsupported);

        let full_group = adw::PreferencesGroup::new();
        let full_row = adw::ActionRow::builder().title(tr("Charge to 100% Now")).build();
        full_row.add_prefix(&gtk::Image::from_icon_name("battery-level-100-charged-symbolic"));
        let full_button = gtk::Button::builder().label(tr("Charge Now")).valign(gtk::Align::Center).build();
        full_row.add_suffix(&full_button);
        full_group.add(&full_row);
        root.add(&full_group);

        let mode_group = adw::PreferencesGroup::builder()
            .title(tr("Charging Behavior"))
            .description(tr("Temporarily pause charging, or run from the battery while still plugged in."))
            .visible(false)
            .build();
        let mode = adw::ComboRow::builder().title(tr("While Plugged In")).build();
        mode_group.add(&mode);
        root.add(&mode_group);

        let page = Rc::new(ChargingPage {
            root,
            limit_group,
            unsupported,
            status_row,
            profile,
            enable,
            end,
            start,
            persist,
            apply,
            schedule_group,
            optimized,
            learned_row,
            lead,
            add_row,
            save_schedule,
            rule_rows: RefCell::new(Vec::new()),
            plan: RefCell::new(Schedule::default()),
            saved_plan: RefCell::new(Schedule::default()),
            full_group,
            full_row,
            full_button,
            mode_group,
            mode,
            mode_ids: RefCell::new(Vec::new()),
            initialized: Cell::new(false),
            schedule_loaded: Cell::new(false),
            profile_end: Cell::new(None),
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
                this.mark_custom();
            }
        });
        let this = Rc::downgrade(self);
        self.end.connect_value_notify(move |end| {
            if let Some(this) = this.upgrade() {
                this.mark_custom();
                // Keep the resume point below the stop point.
                if this.start.value() >= end.value() {
                    this.start.set_value(end.value() - 5.0);
                }
            }
        });
        let this = Rc::downgrade(self);
        self.start.connect_value_notify(move |_| {
            if let Some(this) = this.upgrade() {
                this.mark_custom();
            }
        });
        let this = Rc::downgrade(self);
        self.full_button.connect_clicked(move |_| {
            let Some(this) = this.upgrade() else { return };
            let Some(ctrl) = this.ctrl.upgrade() else { return };
            if ctrl.state().full_charge {
                ctrl.finish_full_charge();
                this.toast(tr("Back to your charge limit"));
            } else {
                ctrl.start_full_charge();
                this.toast(tr("Charging to 100% once; your limit comes back when full or unplugged"));
            }
        });
        let this = Rc::downgrade(self);
        self.profile.connect_selected_notify(move |row| {
            let Some(this) = this.upgrade() else { return };
            let i = row.selected() as usize;
            row.set_subtitle(profile_hint(i));
            if this.updating.get() {
                return;
            }
            // A preset fills in the settings below and applies them right away;
            // "Custom" leaves them for you to edit.
            let Some(&(stop, resume)) = PROFILES.get(i) else { return };
            this.updating.set(true);
            this.enable.set_active(stop < 100);
            if stop < 100 {
                this.end.set_value(stop as f64);
                this.start.set_value(resume as f64);
            }
            this.updating.set(false);
            this.sync_sensitivity();
            this.apply_limit();
        });
        let this = Rc::downgrade(self);
        self.optimized.connect_active_notify(move |row| {
            if let Some(this) = this.upgrade() {
                if !this.updating.get() {
                    this.plan.borrow_mut().optimized = row.is_active();
                    this.plan_changed();
                }
            }
        });
        let this = Rc::downgrade(self);
        self.lead.connect_value_notify(move |row| {
            if let Some(this) = this.upgrade() {
                if !this.updating.get() {
                    this.plan.borrow_mut().lead_min = (row.value() * 60.0).round() as u16;
                    this.plan_changed();
                }
            }
        });
        let this = Rc::downgrade(self);
        self.add_row.connect_activated(move |_| {
            if let Some(this) = this.upgrade() {
                this.ask_for_time();
            }
        });
        let this = Rc::downgrade(self);
        self.save_schedule.connect_clicked(move |_| {
            if let Some(this) = this.upgrade() {
                this.save_plan();
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
                this.run_helper(vec![vec!["set-behaviour".into(), this.battery_name(), id.clone()]], trf("Charging behavior set to {0}", &[("0", behaviour_label(&id).to_string())]));
            }
        });
    }

    /// Shows the edited schedule's times and enables Save when it differs from the saved one.
    fn plan_changed(self: &Rc<Self>) {
        self.rebuild_rules();
        self.save_schedule.set_sensitive(*self.plan.borrow() != *self.saved_plan.borrow() && !self.busy.get());
    }

    fn rebuild_rules(self: &Rc<Self>) {
        for row in self.rule_rows.borrow_mut().drain(..) {
            self.schedule_group.remove(&row);
        }
        // Keep "Add a Time" last.
        self.schedule_group.remove(&self.add_row);
        self.schedule_group.remove(&self.save_schedule);
        let rules = self.plan.borrow().rules.clone();
        for (i, rule) in rules.iter().enumerate() {
            let row = adw::ActionRow::builder()
                .title(trf("Full by {time}", &[("time", schedule::format_time(rule.ready))]))
                .subtitle(days_label(rule.days))
                .build();
            row.add_prefix(&gtk::Image::from_icon_name("alarm-symbolic"));
            let remove = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text(tr("Remove"))
                .valign(gtk::Align::Center)
                .css_classes(["flat"])
                .build();
            let this = Rc::downgrade(self);
            remove.connect_clicked(move |_| {
                if let Some(this) = this.upgrade() {
                    if i < this.plan.borrow().rules.len() {
                        this.plan.borrow_mut().rules.remove(i);
                    }
                    this.plan_changed();
                }
            });
            row.add_suffix(&remove);
            self.schedule_group.add(&row);
            self.rule_rows.borrow_mut().push(row);
        }
        self.add_row.set_visible(rules.len() < schedule::MAX_RULES);
        self.schedule_group.add(&self.add_row);
        self.schedule_group.add(&self.save_schedule);
    }

    /// Asks for the days and the time of a new "full by" entry.
    fn ask_for_time(self: &Rc<Self>) {
        let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).build();
        let days_box = gtk::Box::builder().css_classes(["linked"]).halign(gtk::Align::Center).build();
        let day_buttons: Vec<gtk::ToggleButton> = (0..7u8)
            .map(|i| {
                let b = gtk::ToggleButton::builder().label(weekday_name(i)).active(i < 5).build();
                days_box.append(&b);
                b
            })
            .collect();
        let two_digits = |spin: &gtk::SpinButton| {
            spin.connect_output(|s| {
                s.set_text(&format!("{:02}", s.value() as u32));
                glib::Propagation::Stop
            });
        };
        let hours = gtk::SpinButton::with_range(0.0, 23.0, 1.0);
        let minutes = gtk::SpinButton::with_range(0.0, 55.0, 5.0);
        hours.set_value(8.0);
        hours.set_wrap(true);
        minutes.set_wrap(true);
        two_digits(&hours);
        two_digits(&minutes);
        let time_box = gtk::Box::builder().spacing(6).halign(gtk::Align::Center).build();
        time_box.append(&hours);
        time_box.append(&gtk::Label::new(Some(":")));
        time_box.append(&minutes);
        content.append(&days_box);
        content.append(&time_box);

        let dialog = adw::AlertDialog::builder()
            .heading(tr("Add a Time"))
            .body(tr("Choose the days, and the time you want the battery to be full by."))
            .extra_child(&content)
            .build();
        dialog.add_responses(&[("cancel", tr("Cancel")), ("add", tr("Add"))]);
        dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("add"));
        let this = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            let Some(this) = this.upgrade() else { return };
            if response != "add" {
                return;
            }
            let days = day_buttons.iter().enumerate().filter(|(_, b)| b.is_active()).fold(0u8, |m, (i, _)| m | 1 << i);
            if days == 0 {
                this.toast(tr("Choose at least one day"));
                return;
            }
            let ready = hours.value() as u16 * 60 + minutes.value() as u16;
            let rule = Rule { days, ready };
            {
                let mut plan = this.plan.borrow_mut();
                if !plan.rules.contains(&rule) && plan.rules.len() < schedule::MAX_RULES {
                    plan.rules.push(rule);
                    plan.rules.sort_by_key(|r| (r.ready, r.days));
                }
            }
            this.plan_changed();
        });
        dialog.present(Some(&self.root));
    }

    fn save_plan(self: &Rc<Self>) {
        let plan = self.plan.borrow().clone();
        let mut args: Vec<String> = vec!["set-schedule".into(), u8::from(plan.optimized).to_string(), plan.lead_min.to_string()];
        args.extend(plan.rules.iter().map(|r| format!("{}@{}", r.days, schedule::format_time(r.ready))));
        let msg = if plan.is_empty() { tr("Schedule removed").to_string() } else { tr("Schedule saved").to_string() };
        self.run_helper(vec![args], msg);
    }

    /// Editing the settings by hand no longer matches a preset.
    fn mark_custom(&self) {
        if !self.updating.get() {
            self.updating.set(true);
            self.profile.set_selected(CUSTOM_PROFILE as u32);
            self.updating.set(false);
        }
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
        let msg = if on { trf("Charging will stop at {end}%", &[("end", end.to_string())]) } else { tr("Charge limit removed").to_string() };
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
        self.save_schedule.set_sensitive(false);
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
            this.save_schedule.set_sensitive(*this.plan.borrow() != *this.saved_plan.borrow());
            match result {
                Ok(()) => this.toast(&success),
                Err(e) => this.toast(&e),
            }
            if let Some(ctrl) = this.ctrl.upgrade() {
                ctrl.refresh();
            }
        });
    }

    pub fn update(self: &Rc<Self>, st: &State) {
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
            self.full_row.set_title(tr("Charging to 100%"));
            self.full_row.set_subtitle(tr("Your limit comes back when the battery is full or you unplug."));
            self.full_button.set_label(tr("Cancel"));
            self.full_button.set_sensitive(true);
            self.full_button.remove_css_class("suggested-action");
        } else {
            self.full_row.set_title(tr("Charge to 100% Now"));
            self.full_row.set_subtitle(if b.ac_online {
                tr("Charge fully once, for a trip or a long day. Your limit comes back when full or unplugged.")
            } else {
                tr("Connect the charger to charge fully once.")
            });
            self.full_button.set_label(tr("Charge Now"));
            self.full_button.set_sensitive(b.ac_online);
        }
        // Follow the saved limit, but leave the choice alone while you are
        // editing the settings by hand.
        if self.profile_end.replace(Some(end)) != Some(end) {
            self.profile.set_selected(profile_for(end) as u32);
        }
        let status = match (end < 100, b.start_threshold.filter(|s| *s > 0)) {
            (true, Some(s)) => trf("Stops at {end}%, resumes below {s}%", &[("end", end.to_string()), ("s", s.to_string())]),
            (true, None) => trf("Stops at {end}%", &[("end", end.to_string())]),
            (false, _) => tr("Charges to 100%").to_string(),
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

        // The schedule: load what is saved, unless you are in the middle of editing.
        let saved = system::read_schedule();
        let editing = *self.plan.borrow() != *self.saved_plan.borrow();
        if !self.schedule_loaded.get() || (!editing && saved != *self.saved_plan.borrow()) {
            self.schedule_loaded.set(true);
            *self.saved_plan.borrow_mut() = saved.clone();
            *self.plan.borrow_mut() = saved.clone();
            self.optimized.set_active(saved.optimized);
            self.lead.set_value(saved.lead_min as f64 / 60.0);
            self.rebuild_rules();
            self.save_schedule.set_sensitive(false);
        }
        self.schedule_group.set_visible(supported);
        let has_limit = system::has_saved_limit();
        self.optimized.set_sensitive(has_limit);
        self.lead.set_sensitive(has_limit);
        self.add_row.set_sensitive(has_limit);
        self.learned_row.set_visible(self.optimized.is_active());
        if self.optimized.is_active() {
            self.learned_row.set_subtitle(&learned_text(&system::read_learned()));
        }
        if system::scheduled_full_active(&b.name) {
            self.status_row.set_subtitle(tr("Charging to 100% for your schedule; your limit comes back afterwards"));
        }

        // Charging behavior (charge_behaviour)
        let has_modes = b.charge_behaviours.len() > 1;
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
