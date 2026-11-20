//! History retention and explicit local backup/restore/clear actions.
use crate::controller::Controller;
use crate::i18n::{tr, trnf};
use crate::settings::Settings;
use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::Cell;
use std::rc::Rc;

fn error(parent: &impl IsA<gtk::Widget>, message: &str) {
    let dialog = adw::AlertDialog::builder()
        .heading(tr("Could Not Update History"))
        .body(message)
        .build();
    dialog.add_response("close", tr("Close"));
    dialog.present(Some(parent));
}
fn days_label(days: u16) -> String {
    if days == 0 {
        tr("Keep All History").into()
    } else {
        trnf(
            "{n} day",
            "{n} days",
            u64::from(days),
            &[("n", days.to_string())],
        )
    }
}

fn retention(
    ctrl: &Rc<Controller>,
    title: &str,
    get: fn(&Settings) -> u16,
    set: fn(&mut Settings, u16),
) -> adw::ComboRow {
    let current = get(&ctrl.state().settings);
    let mut choices = vec![7, 30, 90, 365, 0];
    if !choices.contains(&current) {
        choices.insert(0, current);
    }
    let labels: Vec<String> = choices.iter().map(|d| days_label(*d)).collect();
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title(title)
        .use_subtitle(true)
        .model(&gtk::StringList::new(&refs))
        .selected(choices.iter().position(|d| *d == current).unwrap_or(0) as u32)
        .build();
    let updating = Rc::new(Cell::new(false));
    let weak = Rc::downgrade(ctrl);
    row.connect_selected_notify(move |row| {
        if updating.get() { return; }
        let Some(ctrl) = weak.upgrade() else { return };
        let Some(&days) = choices.get(row.selected() as usize) else { return };
        let old = get(&ctrl.state().settings);
        if old == days { return; }
        let (row, choices, updating) = (row.clone(), choices.clone(), updating.clone());
        let parent = row.clone();
        let apply = move |confirmed: bool| {
            if confirmed { ctrl.update_settings(|s| set(s, days)); }
            let saved = get(&ctrl.state().settings);
            updating.set(true);
            row.set_selected(choices.iter().position(|d| *d == saved).unwrap_or(0) as u32);
            updating.set(false); row.set_sensitive(true);
            let failure = ctrl.state().settings_error.clone();
            if let Some(message) = failure { error(&row, &message); }
        };
        if days != 0 && (old == 0 || days < old) {
            let dialog = adw::AlertDialog::builder().heading(tr("Shorten History Retention?"))
                .body(tr("Older records in this category will be permanently removed. Back up your history first if you want to keep them."))
                .build();
            dialog.add_responses(&[("cancel", tr("Cancel")), ("apply", tr("Apply Retention"))]);
            dialog.set_response_appearance("apply", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel")); dialog.set_close_response("cancel");
            parent.set_sensitive(false);
            dialog.connect_response(None, move |_, response| apply(response == "apply"));
            dialog.present(Some(&parent));
        } else { apply(true); }
    });
    row
}

fn action(title: &str, subtitle: &str, icon: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .activatable(true)
        .build();
    row.add_prefix(&super::widgets::decorative_icon(icon));
    row.add_suffix(&super::widgets::decorative_icon("go-next-symbolic"));
    row
}

pub fn add(page: &adw::PreferencesPage, dialog: &adw::PreferencesDialog, ctrl: &Rc<Controller>) {
    let retention_group = adw::PreferencesGroup::builder().title(tr("History Retention"))
        .description(tr("Choose how long to keep each category. Shortening a period removes older records after confirmation."))
        .build();
    retention_group.add(&retention(
        ctrl,
        tr("Battery History"),
        |s| s.history_days,
        |s, v| s.history_days = v,
    ));
    retention_group.add(&retention(
        ctrl,
        tr("App Activity"),
        |s| s.app_history_days,
        |s, v| s.app_history_days = v,
    ));
    retention_group.add(&retention(
        ctrl,
        tr("Battery Health History"),
        |s| s.health_history_days,
        |s, v| s.health_history_days = v,
    ));
    page.add(&retention_group);
    let group = adw::PreferencesGroup::builder().title(tr("Manage History"))
        .description(tr("Backups contain battery history, app names and health records. Charge limits, charging schedules and preferences are not included."))
        .build();
    let backup = action(
        tr("Back Up History…"),
        tr("Save a local history backup file"),
        "document-save-symbolic",
    );
    let restore = action(
        tr("Restore History…"),
        tr("Replace recorded history from a validated backup"),
        "document-open-symbolic",
    );
    let clear = action(
        tr("Clear History…"),
        tr("Choose the categories to permanently remove"),
        "user-trash-symbolic",
    );
    group.add(&backup);
    group.add(&restore);
    group.add(&clear);
    page.add(&group);
    let weak = Rc::downgrade(ctrl);
    let prefs = dialog.downgrade();
    backup.connect_activated(move |row| {
        let Some(ctrl) = weak.upgrade() else { return };
        let chooser = gtk::FileDialog::builder()
            .title(tr("Back Up History"))
            .initial_name("battery-health-history.bhc")
            .build();
        let parent = row.root().and_downcast::<gtk::Window>();
        let (row, prefs) = (row.clone(), prefs.clone());
        chooser.save(parent.as_ref(), gio::Cancellable::NONE, move |result| {
            let Ok(file) = result else { return };
            match ctrl.backup_history().and_then(|text| {
                file.replace_contents(
                    text.as_bytes(),
                    None,
                    false,
                    gio::FileCreateFlags::REPLACE_DESTINATION,
                    gio::Cancellable::NONE,
                )
                .map(|_| ())
                .map_err(|e| e.to_string())
            }) {
                Ok(()) => {
                    if let Some(prefs) = prefs.upgrade() {
                        prefs.add_toast(adw::Toast::new(tr("History backup saved")));
                    }
                }
                Err(message) => error(&row, &message),
            }
        });
    });
    let weak = Rc::downgrade(ctrl);
    let prefs = dialog.downgrade();
    restore.connect_activated(move |row| {
        let Some(ctrl) = weak.upgrade() else { return };
        let chooser = gtk::FileDialog::builder().title(tr("Restore History")).build();
        let parent = row.root().and_downcast::<gtk::Window>();
        let (row, prefs) = (row.clone(), prefs.clone());
        chooser.open(parent.as_ref(), gio::Cancellable::NONE, move |result| {
            let Ok(file) = result else { return };
            let Some(path) = file.path() else { error(&row, tr("Choose a local backup file.")); return };
            row.set_sensitive(false);
            glib::spawn_future_local(async move {
                let data = gio::spawn_blocking(move || crate::history_store::read_backup(&path)).await;
                row.set_sensitive(true);
                let data = match data { Ok(Ok(data)) => data, Ok(Err(message)) => { error(&row, &message); return }, Err(_) => { error(&row, tr("The backup could not be read.")); return } };
                let confirm = adw::AlertDialog::builder().heading(tr("Replace All Recorded History?"))
                    .body(tr("This replaces battery history, app activity and health records with the backup. Existing history will be removed. Back it up first if you want to keep it."))
                    .build();
                confirm.add_responses(&[("cancel", tr("Cancel")), ("restore", tr("Restore History"))]);
                confirm.set_response_appearance("restore", adw::ResponseAppearance::Destructive);
                confirm.set_default_response(Some("cancel")); confirm.set_close_response("cancel");
                let parent = row.clone();
                confirm.connect_response(None, move |_, response| {
                    if response == "restore" {
                        match ctrl.restore_history(&data) {
                            Ok(()) => if let Some(prefs) = prefs.upgrade() { prefs.add_toast(adw::Toast::new(tr("History restored"))); },
                            Err(message) => error(&parent, &message),
                        }
                    }
                });
                confirm.present(Some(&row));
            });
        });
    });
    let weak = Rc::downgrade(ctrl);
    let prefs = dialog.downgrade();
    clear.connect_activated(move |row| {
        let Some(ctrl) = weak.upgrade() else { return };
        let body = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).build();
        let choices: Vec<_> = [(tr("Battery History"), "history.csv"), (tr("App Activity"), "app-history.csv"), (tr("Battery Health History"), "health.csv")]
            .into_iter().map(|(label, name)| (gtk::CheckButton::with_label(label), name)).collect();
        for (check, _) in &choices { body.append(check); }
        let confirm = adw::AlertDialog::builder().heading(tr("Clear Selected History?"))
            .body(tr("Select the categories to permanently remove. Recording will continue with new samples. Back up your history first if you want to keep it."))
            .extra_child(&body).build();
        confirm.add_responses(&[("cancel", tr("Cancel")), ("clear", tr("Clear Selected History"))]);
        confirm.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
        confirm.set_default_response(Some("cancel")); confirm.set_close_response("cancel"); confirm.set_response_enabled("clear", false);
        for (check, _) in &choices {
            let checks: Vec<_> = choices.iter().map(|(c, _)| c.clone()).collect(); let dialog = confirm.downgrade();
            check.connect_toggled(move |_| { if let Some(dialog) = dialog.upgrade() { dialog.set_response_enabled("clear", checks.iter().any(|c| c.is_active())); } });
        }
        let (parent, prefs) = (row.clone(), prefs.clone());
        confirm.connect_response(None, move |_, response| {
            if response == "clear" {
                let selected: Vec<_> = choices.iter().filter(|(c, _)| c.is_active()).map(|(_, name)| *name).collect();
                match ctrl.clear_history(&selected) {
                    Ok(()) => if let Some(prefs) = prefs.upgrade() { prefs.add_toast(adw::Toast::new(tr("Selected history cleared"))); },
                    Err(message) => error(&parent, &message),
                }
            }
        });
        confirm.present(Some(row));
    });
}
