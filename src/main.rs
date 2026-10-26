mod app_history;
mod apps;
mod battery;
mod controller;
mod health_log;
mod history;
mod report;
mod settings;
mod system;
mod ui;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

pub const APP_ID: &str = "io.github.BatteryHealthCenter";

fn main() -> glib::ExitCode {
    // `--background` (used by the login autostart entry) keeps the app running
    // without a window so battery history keeps being recorded.
    let args: Vec<String> = std::env::args().collect();
    // `--export-report <file>` writes the HTML battery report and exits.
    if let Some(i) = args.iter().position(|a| a == "--export-report") {
        let Some(path) = args.get(i + 1) else {
            eprintln!("usage: battery-health-center --export-report <file.html>");
            return glib::ExitCode::FAILURE;
        };
        let ctrl = controller::Controller::new();
        ctrl.refresh();
        let html = report::build_html(&ctrl.state());
        return match std::fs::write(path, html) {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("could not write {path}: {e}");
                glib::ExitCode::FAILURE
            }
        };
    }
    let background = args.iter().any(|a| a == "--background");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--background").collect();

    let mut flags = gio::ApplicationFlags::empty();
    // Capture mode runs alongside an already-running copy. Inside the snap,
    // AppArmor won't let the app own its ID on the session bus without a
    // dbus slot, which needs a manual store review.
    if std::env::var_os("BHC_SCREENSHOTS").is_some() || system::in_snap() {
        flags |= gio::ApplicationFlags::NON_UNIQUE;
    }
    let app = adw::Application::builder().application_id(APP_ID).flags(flags).build();
    let controller: Rc<RefCell<Option<Rc<controller::Controller>>>> = Rc::default();
    let hold = Rc::new(RefCell::new(None));
    let skip_window = Rc::new(Cell::new(background));

    let c = controller.clone();
    app.connect_startup(move |app| {
        gtk::Window::set_default_icon_name(APP_ID);
        // Some backends (e.g. Broadway) leave the font DPI unset, which breaks
        // libadwaita's sp-based layout widths.
        if let Some(settings) = gtk::Settings::default().filter(|s| s.gtk_xft_dpi() <= 0) {
            settings.set_gtk_xft_dpi(96 * 1024);
        }
        if let Some(display) = gtk::gdk::Display::default() {
            // Lets the app icon show when running from the build tree.
            let icons = gtk::IconTheme::for_display(&display);
            icons.add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/data/icons"));
            // Inside a snap the desktop's icon theme may not exist, and GTK then
            // only falls back to hicolor, which lacks symbolic icons.
            if !icons.has_icon("battery-level-80-symbolic") {
                if let Some(settings) = gtk::Settings::default() {
                    settings.set_gtk_icon_theme_name(Some("Adwaita"));
                }
            }
        }
        let ctrl = controller::Controller::new();
        let weak_app = app.downgrade();
        ctrl.set_notifier(move |id, title, body| {
            if let Some(app) = weak_app.upgrade() {
                let n = gio::Notification::new(title);
                n.set_body(Some(body));
                n.set_icon(&gio::ThemedIcon::new(APP_ID));
                n.set_default_action("app.activate-window");
                app.send_notification(Some(id), &n);
            }
        });
        ctrl.start();
        setup_actions(app, &ctrl);
        *c.borrow_mut() = Some(ctrl);
        if background {
            *hold.borrow_mut() = Some(app.hold());
        }
        // A running app holds back snap updates (for up to about two weeks,
        // after which snapd closes it anyway). When snapd is waiting and only
        // the background copy is running, step aside so the update installs;
        // background recording starts again at the next login.
        if system::in_snap() {
            let weak_app = app.downgrade();
            glib::timeout_add_seconds_local(60, move || {
                let Some(app) = weak_app.upgrade() else { return glib::ControlFlow::Break };
                if system::refresh_pending() && app.windows().is_empty() {
                    let n = gio::Notification::new("Battery Health Center Is Updating");
                    n.set_body(Some("It closed so the update can install. Background recording starts again the next time you log in or open the app."));
                    n.set_icon(&gio::ThemedIcon::new(APP_ID));
                    app.send_notification(Some("updating"), &n);
                    // Give the notification a moment to be delivered.
                    glib::timeout_add_seconds_local_once(3, move || app.quit());
                    return glib::ControlFlow::Break;
                }
                glib::ControlFlow::Continue
            });
        }
    });

    let c = controller.clone();
    app.connect_activate(move |app| {
        if skip_window.replace(false) {
            return;
        }
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        if let Some(ctrl) = c.borrow().as_ref() {
            ui::build_window(app, ctrl).present();
        }
    });

    app.run_with_args(&args)
}

fn setup_actions(app: &adw::Application, ctrl: &Rc<controller::Controller>) {
    let refresh = gio::SimpleAction::new("refresh", None);
    let c = Rc::downgrade(ctrl);
    refresh.connect_activate(move |_, _| {
        if let Some(c) = c.upgrade() {
            c.refresh();
        }
    });

    let about = gio::SimpleAction::new("about", None);
    let a = app.clone();
    about.connect_activate(move |_, _| {
        let dialog = adw::AboutDialog::builder()
            .application_name("Battery Health Center")
            .application_icon(APP_ID)
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("Alexis Rios-Negron")
            .developers(["Alexis Rios-Negron"])
            .copyright("© 2026 Alexis Rios-Negron")
            .comments("Battery health, charge limits and energy usage in one place.")
            .license_type(gtk::License::Gpl30)
            .build();
        dialog.present(a.active_window().as_ref());
    });

    // Clicking a notification opens (or raises) the window.
    let activate_window = gio::SimpleAction::new("activate-window", None);
    let a = app.clone();
    activate_window.connect_activate(move |_, _| a.activate());
    app.add_action(&activate_window);

    let quit = gio::SimpleAction::new("quit", None);
    let a = app.clone();
    quit.connect_activate(move |_, _| a.quit());

    app.add_action(&refresh);
    app.add_action(&about);
    app.add_action(&quit);
    app.set_accels_for_action("app.refresh", &["<Primary>r"]);
    app.set_accels_for_action("win.preferences", &["<Primary>comma"]);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);
}
