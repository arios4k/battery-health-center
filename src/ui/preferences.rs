use std::rc::Rc;

use adw::prelude::*;

use crate::controller::Controller;
use crate::settings::Settings;

/// A switch row bound to one boolean setting.
fn switch(ctrl: &Rc<Controller>, title: &str, subtitle: &str, get: fn(&Settings) -> bool, set: fn(&mut Settings, bool)) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .active(get(&ctrl.state().settings))
        .build();
    let weak = Rc::downgrade(ctrl);
    row.connect_active_notify(move |row| {
        if let Some(ctrl) = weak.upgrade() {
            let on = row.is_active();
            ctrl.update_settings(|s| set(s, on));
        }
    });
    row
}

pub fn show(parent: &impl IsA<gtk::Widget>, ctrl: &Rc<Controller>) {
    let page = adw::PreferencesPage::new();

    let group = adw::PreferencesGroup::builder()
        .title("Notifications")
        .description(
            "Sent while Battery Health Center is running. Turn on Record History in the Background \
             (Usage page) to get them even when the window is closed.",
        )
        .build();
    group.add(&switch(
        ctrl,
        "Charge Limit Reached",
        "When charging stops at your limit",
        |s| s.notify_limit_reached,
        |s, v| s.notify_limit_reached = v,
    ));
    group.add(&switch(
        ctrl,
        "Fully Charged Without a Limit",
        "A reminder to unplug, or to set a charge limit",
        |s| s.notify_full_no_limit,
        |s, v| s.notify_full_no_limit = v,
    ));
    let low = switch(ctrl, "Low Battery", "When the battery drops to the level below", |s| s.notify_low, |s, v| s.notify_low = v);
    group.add(&low);
    let level = adw::SpinRow::builder()
        .title("Low Battery Level")
        .subtitle("Percent")
        .adjustment(&gtk::Adjustment::new(ctrl.state().settings.low_level as f64, 5.0, 50.0, 1.0, 5.0, 0.0))
        .build();
    low.bind_property("active", &level, "sensitive").sync_create().build();
    let weak = Rc::downgrade(ctrl);
    level.connect_value_notify(move |row| {
        if let Some(ctrl) = weak.upgrade() {
            let v = row.value() as u8;
            ctrl.update_settings(|s| s.low_level = v);
        }
    });
    group.add(&level);
    group.add(&switch(
        ctrl,
        "Apps Using Significant Energy",
        "When an app keeps the CPU busy for a while on battery",
        |s| s.notify_app_drain,
        |s, v| s.notify_app_drain = v,
    ));
    group.add(&switch(
        ctrl,
        "High Drain During Sleep",
        "When your laptop used a lot of battery while asleep",
        |s| s.notify_sleep_drain,
        |s, v| s.notify_sleep_drain = v,
    ));
    page.add(&group);

    let dialog = adw::PreferencesDialog::new();
    dialog.add(&page);
    dialog.present(Some(parent));
}
