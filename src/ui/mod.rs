mod app_trend;
mod charging;
mod health;
mod overview;
mod power;
mod preferences;
mod screenshots;
mod usage;
mod widgets;

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::controller::Controller;
use crate::i18n::{tr, trc, trf, trnf};

pub fn build_window(app: &adw::Application, ctrl: &Rc<Controller>) -> adw::ApplicationWindow {
    let stack = adw::ViewStack::new();
    let toasts = adw::ToastOverlay::new();

    let overview = overview::OverviewPage::new(ctrl, &stack);
    let health = health::HealthPage::new(ctrl);
    let charging = charging::ChargingPage::new(ctrl, &toasts);
    let power = power::PowerPage::new(ctrl);
    let usage = usage::UsagePage::new(ctrl);

    stack.add_titled_with_icon(&overview.root, Some("overview"), trc("page title", "Overview"), "battery-level-80-symbolic");
    stack.add_titled_with_icon(&health.root, Some("health"), trc("page title", "Health"), "security-high-symbolic");
    stack.add_titled_with_icon(&charging.root, Some("charging"), trc("page title", "Charging"), "battery-level-80-charging-symbolic");
    stack.add_titled_with_icon(&power.root, Some("power"), trc("page title", "Power"), "system-shutdown-symbolic");
    stack.add_titled_with_icon(&usage.root, Some("usage"), trc("page title", "Usage"), "power-profile-balanced-symbolic");

    let no_battery = adw::StatusPage::builder()
        .icon_name("battery-missing-symbolic")
        .title(tr("No Battery Found"))
        .description(if crate::system::in_snap() {
            tr("Battery Health Center needs a laptop battery to show information. If this laptop has one, make sure the hardware-observe permission is connected.")
        } else {
            tr("Battery Health Center needs a laptop battery to show information.")
        })
        .build();
    let content = gtk::Stack::new();
    content.add_named(&stack, Some("main"));
    content.add_named(&no_battery, Some("empty"));
    toasts.set_child(Some(&content));

    let switcher = adw::ViewSwitcher::builder().stack(&stack).policy(adw::ViewSwitcherPolicy::Wide).build();
    let header = adw::HeaderBar::builder().title_widget(&switcher).build();

    let menu = gio::Menu::new();
    let section = gio::Menu::new();
    section.append(Some(tr("_Refresh")), Some("app.refresh"));
    section.append(Some(tr("_Export Battery Report…")), Some("win.export-report"));
    section.append(Some(tr("Export _Data as CSV…")), Some("win.export-csv"));
    menu.append_section(None, &section);
    let section = gio::Menu::new();
    section.append(Some(tr("_Preferences")), Some("win.preferences"));
    section.append(Some(tr("_About Battery Health Center")), Some("app.about"));
    section.append(Some(tr("_Quit")), Some("app.quit"));
    menu.append_section(None, &section);
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .primary(true)
        .tooltip_text(tr("Main Menu"))
        .build();
    header.pack_end(&menu_button);

    let switcher_bar = adw::ViewSwitcherBar::builder().stack(&stack).build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    if let Some(banner) = permissions_banner() {
        toolbar.add_top_bar(&banner);
    }
    toolbar.add_bottom_bar(&switcher_bar);
    toolbar.set_content(Some(&toasts));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Battery Health Center")
        .default_width(820)
        .default_height(760)
        .width_request(360)
        .height_request(400)
        .content(&toolbar)
        .build();

    let prefs = gio::SimpleAction::new("preferences", None);
    let (w, c) = (window.downgrade(), Rc::downgrade(ctrl));
    prefs.connect_activate(move |_, _| {
        if let (Some(w), Some(c)) = (w.upgrade(), c.upgrade()) {
            preferences::show(&w, &c);
        }
    });
    window.add_action(&prefs);

    let export = gio::SimpleAction::new("export-report", None);
    let (w, c, t) = (window.downgrade(), Rc::downgrade(ctrl), toasts.clone());
    export.connect_activate(move |_, _| {
        if let (Some(w), Some(c)) = (w.upgrade(), c.upgrade()) {
            export_report(&w, &c, &t);
        }
    });
    window.add_action(&export);

    let export_csv = gio::SimpleAction::new("export-csv", None);
    let (w, c, t) = (window.downgrade(), Rc::downgrade(ctrl), toasts.clone());
    export_csv.connect_activate(move |_, _| {
        if let (Some(w), Some(c)) = (w.upgrade(), c.upgrade()) {
            export_csv_files(&w, &c, &t);
        }
    });
    window.add_action(&export_csv);

    // On narrow windows, move the page switcher to the bottom.
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 600sp").unwrap());
    breakpoint.add_setter(&switcher_bar, "reveal", Some(&true.to_value()));
    breakpoint.add_setter(&switcher, "visible", Some(&false.to_value()));
    // ...and stack the overview's status text under the gauge.
    breakpoint.add_setter(&overview.hero, "orientation", Some(&gtk::Orientation::Vertical.to_value()));
    breakpoint.add_setter(&overview.hero_text, "halign", Some(&gtk::Align::Center.to_value()));
    window.add_breakpoint(breakpoint);

    if let Some(dir) = std::env::var_os("BHC_SCREENSHOTS") {
        let usage = usage.clone();
        screenshots::capture(&window, &stack, dir.into(), move || usage.scroll_to_apps());
    }

    let weak_window = window.downgrade();
    ctrl.subscribe(move |st| {
        if weak_window.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        content.set_visible_child_name(if st.battery.is_some() { "main" } else { "empty" });
        overview.update(st);
        health.update(st);
        charging.update(st);
        power.update(st);
        usage.update(st);
        glib::ControlFlow::Continue
    });

    window
}

/// In the snap, explains how to connect any interfaces that aren't connected.
fn permissions_banner() -> Option<adw::Banner> {
    let missing = crate::system::missing_snap_plugs();
    if missing.is_empty() {
        return None;
    }
    let banner = adw::Banner::builder()
        .title(tr("Some features need permissions that aren't connected yet"))
        .button_label(tr("How to Fix"))
        .revealed(true)
        .build();
    banner.connect_button_clicked(move |banner| {
        let mut body = String::from(tr("Run these commands in a terminal, then restart Battery Health Center:\n\n"));
        for plug in &missing {
            let purpose = crate::system::SNAP_PLUGS.iter().find(|(p, _)| p == plug).map(|(_, d)| tr(d)).unwrap_or("");
            body.push_str(&format!("sudo snap connect battery-health-center:{plug}\n    (to {purpose})\n"));
        }
        let dialog = adw::AlertDialog::builder().heading(tr("Connect Permissions")).body(body.trim_end()).build();
        dialog.set_body_use_markup(false);
        dialog.add_response("close", tr("Close"));
        dialog.present(Some(banner));
    });
    Some(banner)
}

/// Asks where to save the HTML battery report, then writes it.
fn export_report(window: &adw::ApplicationWindow, ctrl: &Rc<Controller>, toasts: &adw::ToastOverlay) {
    let html = crate::report::build_html(&ctrl.state());
    let today = glib::DateTime::now_local().and_then(|d| d.format("%Y-%m-%d")).map(|d| d.to_string()).unwrap_or_default();
    let dialog = gtk::FileDialog::builder()
        .title(tr("Export Battery Report"))
        .initial_name(format!("battery-report-{today}.html"))
        .build();
    let (w, toasts) = (window.downgrade(), toasts.clone());
    dialog.save(Some(window), gio::Cancellable::NONE, move |result| {
        let Ok(file) = result else { return }; // cancelled
        let written = file.replace_contents(
            html.as_bytes(),
            None,
            false,
            gio::FileCreateFlags::REPLACE_DESTINATION,
            gio::Cancellable::NONE,
        );
        let toast = match written {
            Ok(_) => {
                let toast = adw::Toast::builder().title(tr("Battery report saved")).button_label(tr("Open")).build();
                let w = w.clone();
                toast.connect_button_clicked(move |_| {
                    gtk::FileLauncher::new(Some(&file)).launch(w.upgrade().as_ref(), gio::Cancellable::NONE, |_| {});
                });
                toast
            }
            Err(e) => adw::Toast::new(&trf("Could not save the report: {0}", &[("0", e.message().to_string())])),
        };
        toasts.add_toast(toast);
    });
}

/// Asks for a folder, then writes the history as CSV files in it.
fn export_csv_files(window: &adw::ApplicationWindow, ctrl: &Rc<Controller>, toasts: &adw::ToastOverlay) {
    let dialog = gtk::FileDialog::builder().title(tr("Export Data as CSV")).accept_label(tr("Save Here")).build();
    let (w, ctrl, toasts) = (window.downgrade(), ctrl.clone(), toasts.clone());
    dialog.select_folder(Some(window), gio::Cancellable::NONE, move |result| {
        let Ok(folder) = result else { return }; // cancelled
        let Some(dir) = folder.path() else { return };
        let iso = |ts: i64| {
            glib::DateTime::from_unix_local(ts).and_then(|d| d.format("%Y-%m-%dT%H:%M:%S%:z")).map(|s| s.to_string()).unwrap_or_default()
        };
        let toast = match crate::export::write_all(&dir, &ctrl.state(), &iso) {
            Ok(files) => {
                let toast = adw::Toast::builder()
                    .title(trnf("Saved {n} file", "Saved {n} files", files.len() as u64, &[("n", files.len().to_string())]))
                    .button_label(tr("Open"))
                    .build();
                let w = w.clone();
                toast.connect_button_clicked(move |_| {
                    gtk::FileLauncher::new(Some(&folder)).launch(w.upgrade().as_ref(), gio::Cancellable::NONE, |_| {});
                });
                toast
            }
            Err(e) => adw::Toast::new(&trf("Could not save the files: {0}", &[("0", e.to_string())])),
        };
        toasts.add_toast(toast);
    });
}
