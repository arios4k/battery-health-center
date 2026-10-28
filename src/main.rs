mod app_history;
mod apps;
mod battery;
mod controller;
mod desktop;
mod export;
mod health_log;
mod history;
mod i18n;
#[allow(dead_code)]
mod schedule;
mod report;
mod settings;
mod system;
mod ui;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use crate::i18n::tr;

pub const APP_ID: &str = "io.github.BatteryHealthCenter";

/// An upgrade replaces the program file under the hidden background copy, which
/// would keep running the old version (and make the app open as the old version)
/// until the next login. Once the file has been replaced and no window is open,
/// start the new version and step aside.
fn restart_after_upgrade(app: &adw::Application) {
    let weak = app.downgrade();
    glib::timeout_add_seconds_local(60, move || {
        let Some(app) = weak.upgrade() else { return glib::ControlFlow::Break };
        if !app.windows().is_empty() {
            return glib::ControlFlow::Continue;
        }
        // Linux marks the link of a replaced program file with " (deleted)".
        let Ok(exe) = std::fs::read_link("/proc/self/exe") else { return glib::ControlFlow::Continue };
        let exe = exe.to_string_lossy().into_owned();
        let Some(path) = exe.strip_suffix(" (deleted)") else { return glib::ControlFlow::Continue };
        if !std::path::Path::new(path).is_file() {
            return glib::ControlFlow::Continue; // the new file isn't in place yet
        }
        // The new copy can only start after this one has given up the application ID.
        let started = std::process::Command::new("sh")
            .args(["-c", "sleep 3; exec \"$0\" --background", path])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        if started.is_ok() {
            app.quit();
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}

fn main() -> glib::ExitCode {
    i18n::init();
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
    // `--export-csv <folder>` writes the history as CSV files and exits.
    if let Some(i) = args.iter().position(|a| a == "--export-csv") {
        let Some(dir) = args.get(i + 1) else {
            eprintln!("usage: battery-health-center --export-csv <folder>");
            return glib::ExitCode::FAILURE;
        };
        let ctrl = controller::Controller::new();
        ctrl.refresh();
        let iso = |ts: i64| glib::DateTime::from_unix_local(ts).and_then(|d| d.format("%Y-%m-%dT%H:%M:%S%:z")).map(|s| s.to_string()).unwrap_or_default();
        let dir = std::path::Path::new(dir);
        return match std::fs::create_dir_all(dir).and_then(|()| export::write_all(dir, &ctrl.state(), &iso)) {
            Ok(files) => {
                for f in files {
                    println!("{}", f.display());
                }
                glib::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("could not write the files: {e}");
                glib::ExitCode::FAILURE
            }
        };
    }
    let background = args.iter().any(|a| a == "--background");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--background").collect();

    let mut flags = gio::ApplicationFlags::empty();
    // Capture mode runs alongside an already-running copy.
    if std::env::var_os("BHC_SCREENSHOTS").is_some() {
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
            restart_after_upgrade(app);
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
            .comments(tr("Battery health, charge limits and energy usage in one place."))
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
