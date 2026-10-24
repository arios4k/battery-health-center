mod charging;
mod health;
mod overview;
mod screenshots;
mod usage;
mod widgets;

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::controller::Controller;

pub fn build_window(app: &adw::Application, ctrl: &Rc<Controller>) -> adw::ApplicationWindow {
    let stack = adw::ViewStack::new();
    let toasts = adw::ToastOverlay::new();

    let overview = overview::OverviewPage::new(ctrl, &stack);
    let health = Rc::new(health::HealthPage::new());
    let charging = charging::ChargingPage::new(ctrl, &toasts);
    let usage = usage::UsagePage::new(ctrl);

    stack.add_titled_with_icon(&overview.root, Some("overview"), "Overview", "battery-level-80-symbolic");
    stack.add_titled_with_icon(&health.root, Some("health"), "Health", "security-high-symbolic");
    stack.add_titled_with_icon(&charging.root, Some("charging"), "Charging", "battery-level-80-charging-symbolic");
    stack.add_titled_with_icon(&usage.root, Some("usage"), "Usage", "power-profile-balanced-symbolic");

    let no_battery = adw::StatusPage::builder()
        .icon_name("battery-missing-symbolic")
        .title("No Battery Found")
        .description(if crate::system::in_snap() {
            "Battery Health Center needs a laptop battery to show information. If this laptop has one, \
             make sure the hardware-observe permission is connected."
        } else {
            "Battery Health Center needs a laptop battery to show information."
        })
        .build();
    let content = gtk::Stack::new();
    content.add_named(&stack, Some("main"));
    content.add_named(&no_battery, Some("empty"));
    toasts.set_child(Some(&content));

    let switcher = adw::ViewSwitcher::builder().stack(&stack).policy(adw::ViewSwitcherPolicy::Wide).build();
    let header = adw::HeaderBar::builder().title_widget(&switcher).build();

    let menu = gio::Menu::new();
    menu.append(Some("_Refresh"), Some("app.refresh"));
    menu.append(Some("_About Battery Health Center"), Some("app.about"));
    menu.append(Some("_Quit"), Some("app.quit"));
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .primary(true)
        .tooltip_text("Main Menu")
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

    // On narrow windows, move the page switcher to the bottom.
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 600sp").unwrap());
    breakpoint.add_setter(&switcher_bar, "reveal", Some(&true.to_value()));
    breakpoint.add_setter(&switcher, "visible", Some(&false.to_value()));
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
        .title("Some features need permissions that aren't connected yet")
        .button_label("How to Fix")
        .revealed(true)
        .build();
    banner.connect_button_clicked(move |banner| {
        let mut body = String::from("Run these commands in a terminal, then restart Battery Health Center:\n\n");
        for plug in &missing {
            let purpose = crate::system::SNAP_PLUGS.iter().find(|(p, _)| p == plug).map(|(_, d)| *d).unwrap_or("");
            body.push_str(&format!("sudo snap connect battery-health-center:{plug}\n    (to {purpose})\n"));
        }
        let dialog = adw::AlertDialog::builder().heading("Connect Permissions").body(body.trim_end()).build();
        dialog.set_body_use_markup(false);
        dialog.add_response("close", "Close");
        dialog.present(Some(banner));
    });
    Some(banner)
}
