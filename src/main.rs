mod apps;
mod battery;
mod controller;
mod history;
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
    let background = args.iter().any(|a| a == "--background");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--background").collect();

    let mut flags = gio::ApplicationFlags::empty();
    if std::env::var_os("BHC_SCREENSHOTS").is_some() {
        // Capture mode runs alongside an already-running copy.
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
        ctrl.start();
        setup_actions(app, &ctrl);
        *c.borrow_mut() = Some(ctrl);
        if background {
            *hold.borrow_mut() = Some(app.hold());
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

    let quit = gio::SimpleAction::new("quit", None);
    let a = app.clone();
    quit.connect_activate(move |_, _| a.quit());

    app.add_action(&refresh);
    app.add_action(&about);
    app.add_action(&quit);
    app.set_accels_for_action("app.refresh", &["<Primary>r"]);
    app.set_accels_for_action("app.quit", &["<Primary>q"]);
}
