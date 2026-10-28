use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::gio;

use crate::controller::{Controller, State};
use crate::desktop;
use crate::system;
use crate::i18n::tr;

/// Suspend delays offered, in seconds.
const SUSPEND_SECS: &[i64] = &[300, 600, 900, 1200, 1800, 2700, 3600, 7200];
/// Screen blank delays offered, in seconds.
const BLANK_SECS: &[i64] = &[60, 120, 180, 240, 300, 480, 600, 720, 900];

fn note(title: &str, why: &str, icon: &str) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder().title(title).subtitle(why).build();
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    group.add(&row);
    group
}

fn bool_row(settings: &gio::Settings, key: &str, title: &str, subtitle: &str) -> Option<adw::SwitchRow> {
    if !desktop::has_key(settings, key) {
        return None;
    }
    let row = adw::SwitchRow::builder().title(title).subtitle(subtitle).build();
    settings.bind(key, &row, "active").build();
    Some(row)
}

/// A drop-down for an enum setting, offering only values the schema allows
/// (Hibernate only when the laptop can hibernate).
fn enum_row(settings: &gio::Settings, key: &str, title: &str, subtitle: &str, hibernate_ok: bool) -> Option<adw::ComboRow> {
    let mut values = desktop::enum_values(settings, key);
    if values.is_empty() {
        return None;
    }
    let current = settings.string(key).to_string();
    values.retain(|v| v != "hibernate" || hibernate_ok || *v == current);
    let labels: Vec<String> = values.iter().map(|v| desktop::action_label(v)).collect();
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder().title(title).subtitle(subtitle).model(&gtk::StringList::new(&refs)).build();

    let values = Rc::new(values);
    let updating = Rc::new(Cell::new(false));
    let sync: Rc<dyn Fn()> = {
        let (row, values, settings, key, updating) =
            (row.clone(), values.clone(), settings.clone(), key.to_string(), updating.clone());
        Rc::new(move || {
            let now = settings.string(&key);
            if let Some(i) = values.iter().position(|v| *v == now) {
                updating.set(true);
                row.set_selected(i as u32);
                updating.set(false);
            }
        })
    };
    sync();
    settings.connect_changed(Some(key), {
        let sync = sync.clone();
        move |_, _| sync()
    });
    let (settings, key) = (settings.clone(), key.to_string());
    row.connect_selected_notify(move |row| {
        if updating.get() {
            return;
        }
        if let Some(v) = values.get(row.selected() as usize).filter(|v| **v != settings.string(&key)) {
            let _ = settings.set_string(&key, v);
        }
    });
    Some(row)
}

/// A drop-down of delays: "Never" plus the given options (and the current
/// value, if it isn't one of them). `current` reads the setting (None = never)
/// and `apply` writes it. `watch` lists the keys whose changes should refresh it.
fn timeout_row(
    title: &str,
    subtitle: &str,
    options: &'static [i64],
    current: Rc<dyn Fn() -> Option<i64>>,
    apply: Rc<dyn Fn(Option<i64>)>,
    watch: &[(&gio::Settings, &str)],
) -> adw::ComboRow {
    let row = adw::ComboRow::builder().title(title).subtitle(subtitle).build();
    let entries: Rc<RefCell<Vec<Option<i64>>>> = Rc::default();
    let updating = Rc::new(Cell::new(false));

    let rebuild: Rc<dyn Fn()> = {
        let (row, entries, updating, current) = (row.clone(), entries.clone(), updating.clone(), current.clone());
        Rc::new(move || {
            let now = current();
            let mut list: Vec<Option<i64>> = std::iter::once(None).chain(options.iter().map(|s| Some(*s))).collect();
            if let Some(c) = now.filter(|c| !options.contains(c)) {
                list.push(Some(c));
                list[1..].sort();
            }
            // Decide first, so no borrow is held while the widgets change.
            let list_changed = *entries.borrow() != list;
            updating.set(true);
            if list_changed {
                let labels: Vec<String> =
                    list.iter().map(|e| e.map_or_else(|| tr("Never").to_string(), desktop::duration_label)).collect();
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                row.set_model(Some(&gtk::StringList::new(&refs)));
            }
            row.set_selected(list.iter().position(|e| *e == now).unwrap_or(0) as u32);
            updating.set(false);
            if list_changed {
                *entries.borrow_mut() = list;
            }
        })
    };
    rebuild();
    for (settings, key) in watch {
        let rebuild = rebuild.clone();
        settings.connect_changed(Some(key), move |_, _| rebuild());
    }
    row.connect_selected_notify(move |row| {
        if updating.get() {
            return;
        }
        // Copy the choice out first: saving it triggers a refresh (via the settings'
        // "changed" signal) that needs `entries` again, so no borrow may be held here.
        let choice = entries.borrow().get(row.selected() as usize).copied();
        if let Some(choice) = choice.filter(|c| *c != current()) {
            apply(choice);
        }
    });
    row
}

fn profile_combo(title: &str) -> adw::ComboRow {
    adw::ComboRow::builder().title(title).build()
}

pub struct PowerPage {
    pub root: adw::PreferencesPage,
    mode_group: adw::PreferencesGroup,
    mode: adw::ComboRow,
    auto_group: adw::PreferencesGroup,
    auto_switch: adw::SwitchRow,
    auto_ac: adw::ComboRow,
    auto_battery: adw::ComboRow,
    low_switch: adw::SwitchRow,
    low_level: adw::SpinRow,
    /// Available power mode ids, in the order the combo rows list them.
    ids: RefCell<Vec<String>>,
    /// Set while the UI is updated programmatically, to ignore change signals.
    updating: Cell<bool>,
    ctrl: Weak<Controller>,
}

impl PowerPage {
    pub fn new(ctrl: &Rc<Controller>) -> Rc<Self> {
        let root = adw::PreferencesPage::new();

        // Power mode: needs power-profiles-daemon.
        let mode_group = adw::PreferencesGroup::builder()
            .title(tr("Power Mode"))
            .description(tr("Power Saver reduces performance to extend battery life. Performance is only offered on some laptops."))
            .visible(false)
            .build();
        let mode = profile_combo(tr("Current Mode"));
        mode_group.add(&mode);
        root.add(&mode_group);

        let auto_group = adw::PreferencesGroup::builder()
            .title(tr("Automatic Power Mode"))
            .description(
                tr("Switches the power mode by itself when you plug in or unplug the charger. You can still change the mode by hand; it's only switched again the next time something changes."),
            )
            .visible(false)
            .build();
        let auto_switch = adw::SwitchRow::builder().title(tr("Switch Automatically")).build();
        let auto_ac = profile_combo(tr("When Plugged In"));
        let auto_battery = profile_combo(tr("On Battery"));
        let low_switch = adw::SwitchRow::builder()
            .title(tr("Use Power Saver When Battery Is Low"))
            .subtitle(tr("Overrides the mode above on battery"))
            .build();
        let low_level = adw::SpinRow::builder()
            .title(tr("Low Battery Level"))
            .subtitle(tr("Percent"))
            .adjustment(&gtk::Adjustment::new(20.0, 5.0, 60.0, 1.0, 5.0, 0.0))
            .build();
        for w in [auto_switch.upcast_ref::<gtk::Widget>(), auto_ac.upcast_ref(), auto_battery.upcast_ref(), low_switch.upcast_ref(), low_level.upcast_ref()] {
            auto_group.add(w);
        }
        root.add(&auto_group);

        if ctrl.profiles.is_none() {
            root.add(&note(
                tr("Power Mode and Automatic Power Mode Aren't Available"),
                tr("Power Profiles Daemon isn't running, so the power mode can't be read or changed."),
                "dialog-information-symbolic",
            ));
        }

        // GNOME's own power settings.
        let power = desktop::settings(desktop::POWER_SCHEMA);
        let session = desktop::settings(desktop::SESSION_SCHEMA);
        let hibernate_ok = power.is_some() && desktop::can_hibernate();

        if let Some(power) = &power {
            let saving = adw::PreferencesGroup::builder()
                .title(tr("Power Saving"))
                .description(tr("GNOME's own settings that save the most battery. The same ones are in Settings → Power."))
                .build();
            let rows: Vec<Option<gtk::Widget>> = vec![
                ctrl.profiles.as_ref().and(
                    bool_row(
                        power,
                        "power-saver-profile-on-low-battery",
                        tr("Power Saver When Battery Is Low"),
                        tr("GNOME switches to Power Saver mode when the battery gets low"),
                    )
                    .map(|r| r.upcast()),
                ),
                bool_row(
                    power,
                    "idle-dim",
                    tr("Dim Screen When Inactive"),
                    tr("The screen is usually the biggest power user, so dimming it saves a lot"),
                )
                .map(|r| r.upcast()),
                desktop::has_ambient_light()
                    .then(|| bool_row(power, "ambient-enabled", tr("Automatic Screen Brightness"), tr("Adjusts the brightness to the light around you")))
                    .flatten()
                    .map(|r| r.upcast()),
                session.as_ref().filter(|s| desktop::has_key(s, "idle-delay")).map(|session| {
                    let (get, set) = (session.clone(), session.clone());
                    timeout_row(
                        tr("Screen Blank"),
                        tr("Turns the screen off after you stop using the laptop"),
                        BLANK_SECS,
                        Rc::new(move || Some(desktop::get_secs(&get, "idle-delay")).filter(|s| *s > 0)),
                        Rc::new(move |choice| {
                            desktop::set_secs(&set, "idle-delay", choice.unwrap_or(0));
                        }),
                        &[(session, "idle-delay")],
                    )
                    .upcast()
                }),
                suspend_row(power, "sleep-inactive-battery", tr("Automatic Suspend on Battery"), tr("Sleeping uses far less power than staying on"))
                    .map(|r| r.upcast()),
                suspend_row(power, "sleep-inactive-ac", tr("Automatic Suspend When Plugged In"), "")
                    .map(|r| r.upcast()),
            ];
            for row in rows.into_iter().flatten() {
                saving.add(&row);
            }
            root.add(&saving);

            let buttons = adw::PreferencesGroup::builder().title(tr("Power Button and Lid")).build();
            for (key, title, subtitle) in [
                ("power-button-action", tr("Power Button"), ""),
                ("lid-close-battery-action", tr("Close the Lid on Battery"), ""),
                ("lid-close-ac-action", tr("Close the Lid When Plugged In"), ""),
            ] {
                if let Some(row) = enum_row(power, key, title, subtitle, hibernate_ok) {
                    buttons.add(&row);
                }
            }
            root.add(&buttons);
        } else {
            root.add(&note(
                tr("Power Saving and Power Button Settings Aren't Available"),
                tr("GNOME's power settings weren't found. They need GNOME Settings Daemon."),
                "dialog-information-symbolic",
            ));
        }

        if let Some(row) = desktop::settings(desktop::INTERFACE_SCHEMA).and_then(|s| {
            bool_row(&s, "show-battery-percentage", tr("Show Battery Percentage"), tr("Shows the percentage next to the battery icon in the top bar"))
        }) {
            let top_bar = adw::PreferencesGroup::builder().title(tr("Top Bar")).build();
            top_bar.add(&row);
            root.add(&top_bar);
        }

        let page = Rc::new(PowerPage {
            root,
            mode_group,
            mode,
            auto_group,
            auto_switch,
            auto_ac,
            auto_battery,
            low_switch,
            low_level,
            ids: RefCell::new(Vec::new()),
            updating: Cell::new(false),
            ctrl: Rc::downgrade(ctrl),
        });
        page.connect_signals();
        page
    }

    /// Saves a preference, unless the page itself is updating the widgets.
    fn change_setting(&self, change: impl FnOnce(&mut crate::settings::Settings)) {
        if self.updating.get() {
            return;
        }
        if let Some(ctrl) = self.ctrl.upgrade() {
            ctrl.update_settings(change);
        }
    }

    fn connect_signals(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        self.mode.connect_selected_notify(move |row| {
            let Some(this) = this.upgrade() else { return };
            if this.updating.get() {
                return;
            }
            let (Some(ctrl), Some(id)) = (this.ctrl.upgrade(), this.ids.borrow().get(row.selected() as usize).cloned()) else {
                return;
            };
            if let Some(profiles) = &ctrl.profiles {
                // A selection that matches the current mode isn't a user choice (the
                // page also selects the current mode itself when it refreshes).
                if profiles.active().as_deref() != Some(id.as_str()) {
                    if let Err(e) = profiles.set_active(&id) {
                        eprintln!("battery-health-center: could not change power mode: {}", e.message());
                    }
                    ctrl.refresh();
                }
            }
        });

        let this = Rc::downgrade(self);
        self.auto_switch.connect_active_notify(move |row| {
            if let Some(this) = this.upgrade() {
                let on = row.is_active();
                this.change_setting(|s| s.auto_profile = on);
            }
        });
        for (combo, plugged_in) in [(&self.auto_ac, true), (&self.auto_battery, false)] {
            let this = Rc::downgrade(self);
            combo.connect_selected_notify(move |row| {
                let Some(this) = this.upgrade() else { return };
                let Some(id) = this.ids.borrow().get(row.selected() as usize).cloned() else { return };
                this.change_setting(|s| if plugged_in { s.profile_ac = id } else { s.profile_battery = id });
            });
        }
        let this = Rc::downgrade(self);
        self.low_switch.connect_active_notify(move |row| {
            if let Some(this) = this.upgrade() {
                let on = row.is_active();
                this.change_setting(|s| s.low_profile = on);
            }
        });
        let this = Rc::downgrade(self);
        self.low_level.connect_value_notify(move |row| {
            if let Some(this) = this.upgrade() {
                let level = row.value() as u8;
                this.change_setting(|s| s.low_profile_level = level);
            }
        });
    }

    pub fn update(&self, st: &State) {
        let Some(ctrl) = self.ctrl.upgrade() else { return };
        let Some(profiles) = &ctrl.profiles else { return };
        let ids = profiles.available();
        let shown = !ids.is_empty();
        self.mode_group.set_visible(shown);
        self.auto_group.set_visible(shown);
        if !shown {
            return;
        }

        self.updating.set(true);
        if *self.ids.borrow() != ids {
            let labels: Vec<&str> = ids.iter().map(|s| system::profile_label(s)).collect();
            for combo in [&self.mode, &self.auto_ac, &self.auto_battery] {
                combo.set_model(Some(&gtk::StringList::new(&labels)));
            }
            *self.ids.borrow_mut() = ids.clone();
        }
        let select = |combo: &adw::ComboRow, id: &str| {
            if let Some(i) = ids.iter().position(|p| p == id) {
                combo.set_selected(i as u32);
            }
        };
        if let Some(active) = profiles.active() {
            select(&self.mode, &active);
        }
        let s = &st.settings;
        self.auto_switch.set_active(s.auto_profile);
        select(&self.auto_ac, &s.profile_ac);
        select(&self.auto_battery, &s.profile_battery);
        self.low_switch.set_active(s.low_profile);
        self.low_level.set_value(s.low_profile_level as f64);
        for w in [self.auto_ac.upcast_ref::<gtk::Widget>(), self.auto_battery.upcast_ref(), self.low_switch.upcast_ref()] {
            w.set_sensitive(s.auto_profile);
        }
        self.low_level.set_sensitive(s.auto_profile && s.low_profile);
        self.updating.set(false);
    }
}

/// Suspend-when-inactive row for a "<prefix>-type" / "<prefix>-timeout" pair.
fn suspend_row(power: &gio::Settings, prefix: &str, title: &str, subtitle: &str) -> Option<adw::ComboRow> {
    let (type_key, secs_key) = (format!("{prefix}-type"), format!("{prefix}-timeout"));
    if !desktop::has_key(power, &type_key) || !desktop::has_key(power, &secs_key) {
        return None;
    }
    let (get, set) = (power.clone(), power.clone());
    let (get_prefix, set_prefix) = (prefix.to_string(), prefix.to_string());
    Some(timeout_row(
        title,
        subtitle,
        SUSPEND_SECS,
        Rc::new(move || desktop::suspend_delay(&get, &get_prefix)),
        Rc::new(move |choice| {
            desktop::set_suspend_delay(&set, &set_prefix, choice);
        }),
        &[(power, &type_key), (power, &secs_key)],
    ))
}
