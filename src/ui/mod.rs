mod about;
mod app_trend;
mod appearance;
mod charging;
mod health;
mod overview;
mod power;
mod preferences;
mod screenshots;
mod usage;
mod widgets;
mod workspace;

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::controller::Controller;
use crate::i18n::{tr, trc, trf, trnf};

pub fn about_dialog() -> adw::Dialog {
    about::dialog()
}

/// The online user manual, published with GitHub Pages from `docs/`.
pub const MANUAL_URL: &str = "https://arios4k.github.io/battery-health-center/";

/// Opens the user manual in the default browser.
pub fn open_manual(parent: Option<&gtk::Window>) {
    gtk::UriLauncher::new(MANUAL_URL).launch(parent, gio::Cancellable::NONE, |result| {
        if let Err(error) = result {
            eprintln!("could not open the user manual: {error}");
        }
    });
}

pub fn build_window(app: &adw::Application, ctrl: &Rc<Controller>) -> adw::ApplicationWindow {
    appearance::install();
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
        .description(tr("Battery Health Center needs a laptop battery to show information."))
        .build();
    let content = gtk::Stack::new();
    content.add_named(&stack, Some("main"));
    content.add_named(&no_battery, Some("empty"));
    toasts.set_child(Some(&content));

    let heading = gtk::Label::builder().xalign(0.0).wrap(true).css_classes(["title-1"]).build();
    let summary = gtk::Label::builder().xalign(0.0).wrap(true).css_classes(["dim-label"]).build();
    let icon = gtk::Image::builder().pixel_size(32).valign(gtk::Align::Start).css_classes(["tool-icon"]).build();
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 6);
    titles.append(&heading);
    titles.append(&summary);
    let hero = gtk::Box::builder().css_classes(["workspace-heading"]).spacing(18).margin_start(24).margin_end(24).margin_top(12).margin_bottom(20).build();
    hero.append(&icon);
    hero.append(&titles);

    let menu = gio::Menu::new();
    let section = gio::Menu::new();
    section.append(Some(tr("_Refresh")), Some("app.refresh"));
    section.append(Some(tr("_Export Battery Report…")), Some("win.export-report"));
    section.append(Some(tr("Export _Data as CSV…")), Some("win.export-csv"));
    menu.append_section(None, &section);
    let section = gio::Menu::new();
    section.append(Some(tr("_Preferences")), Some("win.preferences"));
    section.append(Some(tr("_User Manual")), Some("app.help"));
    section.append(Some(tr("_About Battery Health Center")), Some("app.about"));
    section.append(Some(tr("_Quit")), Some("app.quit"));
    menu.append_section(None, &section);
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .primary(true)
        .tooltip_text(tr("Main Menu"))
        .build();
    widgets::a11y_name(&menu_button, tr("Main Menu"));
    let help_button = gtk::Button::builder().icon_name("help-browser-symbolic").action_name("app.help")
        .tooltip_text(tr("User Manual (F1)")).build();
    widgets::a11y_name(&help_button, tr("User Manual"));
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.pack_end(&menu_button);
    sidebar_header.pack_end(&help_button);
    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["navigation-sidebar"]).build();
    let pages = [
        ("overview", trc("page title", "Overview"), tr("Your battery, charger and recommendations at a glance."), "battery-level-80-symbolic"),
        ("health", trc("page title", "Health"), tr("Understand battery capacity, wear and long-term health."), "security-high-symbolic"),
        ("charging", trc("page title", "Charging"), tr("Protect your battery with charge limits and schedules."), "battery-level-80-charging-symbolic"),
        ("power", trc("page title", "Power"), tr("Choose a power mode and adjust energy-saving settings."), "system-shutdown-symbolic"),
        ("usage", trc("page title", "Usage"), tr("Explore battery history, sleep drain and app energy use."), "power-profile-balanced-symbolic"),
    ];
    for (id, title, description, icon_name) in pages {
        let row_content = gtk::Box::builder().spacing(12).margin_top(2).margin_bottom(2).margin_start(4).build();
        row_content.append(&widgets::decorative_icon(icon_name));
        row_content.append(&gtk::Label::builder().label(title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build());
        let row = gtk::ListBoxRow::builder().child(&row_content).tooltip_text(description).build();
        row.set_widget_name(id);
        widgets::a11y_description(&row, description);
        list.append(&row);
    }
    let sidebar_view = adw::ToolbarView::new();
    sidebar_view.add_css_class("tool-sidebar");
    sidebar_view.add_top_bar(&sidebar_header);
    sidebar_view.set_content(Some(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&list).build()));
    let sidebar = adw::NavigationPage::new(&sidebar_view, "Battery Health Center");
    let toolbar = adw::ToolbarView::new();
    toolbar.add_css_class("tool-workspace");
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.add_top_bar(&hero);
    let save_banner = adw::Banner::builder().title(tr("Some changes could not be saved")).button_label(tr("Details")).build();
    let save_details = Rc::new(std::cell::RefCell::new(String::new()));
    let details = save_details.clone();
    save_banner.connect_button_clicked(move |banner| {
        let dialog = adw::AlertDialog::builder().heading(tr("Could Not Save Data"))
            .body(format!("{}\n\n{}", tr("Check available disk space and folder permissions. Failed preferences were not applied; unsaved history may be lost when the app closes."), details.borrow()))
            .build();
        dialog.add_response("close", tr("Close"));
        dialog.present(Some(banner));
    });
    toolbar.add_top_bar(&save_banner);
    toolbar.set_content(Some(&toasts));
    let workspace = adw::NavigationPage::new(&toolbar, trc("page title", "Overview"));
    let split = adw::NavigationSplitView::builder().sidebar(&sidebar).content(&workspace).build();
    {
        let (stack, split) = (stack.downgrade(), split.downgrade());
        list.connect_row_activated(move |_, row| {
            if let (Some(stack), Some(split)) = (stack.upgrade(), split.upgrade()) {
                stack.set_visible_child_name(&row.widget_name());
                split.set_show_content(true);
            }
        });
    }
    // Keep the heading and sidebar in sync with overview links and screenshot capture too.
    let sync_page = {
        let (list, workspace, split) = (list.clone(), workspace.clone(), split.downgrade());
        move |stack: &adw::ViewStack| {
            let Some(id) = stack.visible_child_name() else { return };
            if let Some((_, title, description, icon_name)) = pages.iter().find(|p| p.0 == id) {
                heading.set_label(title);
                summary.set_label(description);
                icon.set_icon_name(Some(icon_name));
                workspace.set_title(title);
                if let Some(split) = split.upgrade() { split.set_show_content(true); }
                let mut child = list.first_child();
                while let Some(row) = child {
                    if row.widget_name() == id {
                        row.add_css_class("current-page");
                    } else {
                        row.remove_css_class("current-page");
                    }
                    child = row.next_sibling();
                }
            }
        }
    };
    sync_page(&stack);
    stack.connect_visible_child_name_notify(sync_page);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Battery Health Center")
        .default_width(1240)
        .default_height(840)
        .width_request(360)
        .height_request(400)
        .content(&split)
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

    // Narrow windows use libadwaita's sidebar/back navigation.
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 600sp").unwrap());
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);
    if let Some(dir) = std::env::var_os("BHC_SCREENSHOTS") {
        let usage = usage.clone();
        screenshots::capture(&window, &stack, &split, ctrl, dir.into(), move || usage.scroll_to_apps());
    }

    // The first-launch question comes a moment after the window appears.
    {
        let (weak_window, ctrl, usage) = (window.downgrade(), ctrl.clone(), usage.clone());
        glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
            if let Some(window) = weak_window.upgrade() {
                ask_about_background(&window, &ctrl, &usage);
            }
        });
    }

    let weak_window = window.downgrade();
    ctrl.subscribe(move |st| {
        if weak_window.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        content.set_visible_child_name(if st.battery.is_some() { "main" } else { "empty" });
        let errors: Vec<&str> = [st.settings_error.as_deref(), st.history.write_error.as_deref(), st.health_log.write_error.as_deref(), st.app_history.write_error.as_deref()].into_iter().flatten().collect();
        *save_details.borrow_mut() = errors.join("\n\n");
        save_banner.set_revealed(!errors.is_empty());
        overview.update(st);
        health.update(st);
        charging.update(st);
        power.update(st);
        usage.update(st);
        glib::ControlFlow::Continue
    });

    window
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

/// On the very first launch, asks whether to record history in the background, which
/// starts the app hidden at login. Returning users and a "yes" already given are not asked.
fn ask_about_background(window: &adw::ApplicationWindow, ctrl: &Rc<Controller>, usage: &Rc<usage::UsagePage>) {
    if std::env::var_os("BHC_SCREENSHOTS").is_some() || ctrl.state().settings.background_asked {
        return;
    }
    if crate::system::autostart_enabled() {
        ctrl.update_settings(|s| s.background_asked = true);
        return;
    }
    let dialog = adw::AlertDialog::builder()
        .heading(tr("Record History in the Background?"))
        .body(tr("Battery Health Center can start hidden when you log in, so battery history, sleep drain and notifications keep working with the window closed. You can change this any time on the Usage page."))
        .build();
    dialog.add_responses(&[("later", tr("Not Now")), ("enable", tr("Turn On"))]);
    dialog.set_response_appearance("enable", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("enable"));
    dialog.set_close_response("later");
    let (ctrl, usage) = (ctrl.clone(), usage.clone());
    dialog.connect_response(None, move |_, response| {
        let error = if response == "enable" {
            let error = crate::system::set_autostart(true).err();
            usage.sync_background();
            error
        } else { None };
        ctrl.update_settings(|s| s.background_asked = true);
        if let Some(error) = error { ctrl.report_settings_error(error.to_string()); }
    });
    dialog.present(Some(window));
}
