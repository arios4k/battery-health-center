use std::cell::Cell;
use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::widgets::draw_history;
use crate::apps::AppUsage;
use crate::controller::{Controller, State};
use crate::history::now_unix;
use crate::system;

const MAX_APPS: usize = 8;

pub struct UsagePage {
    pub root: adw::PreferencesPage,
    chart: gtk::DrawingArea,
    now_group: adw::PreferencesGroup,
    now_list: gtk::ListBox,
    avg_list: gtk::ListBox,
    last_apps_gen: Cell<u64>,
    last_history_gen: Cell<u64>,
}

fn impact_label(impact: f64) -> &'static str {
    if impact >= 40.0 {
        "High"
    } else if impact >= 10.0 {
        "Medium"
    } else {
        "Low"
    }
}

fn app_row(app: &AppUsage, show_processes: bool) -> adw::ActionRow {
    let mut subtitle = Vec::new();
    if show_processes && app.processes > 1 {
        subtitle.push(format!("{} processes", app.processes));
    }
    if app.system {
        subtitle.push("System".to_string());
    }
    subtitle.push(format!("{:.0}% CPU", app.impact));
    let row = adw::ActionRow::builder()
        .title(&app.name)
        .subtitle(subtitle.join(" · "))
        .use_markup(false)
        .build();
    let icon = match &app.icon {
        Some(icon) => gtk::Image::from_gicon(icon),
        None => gtk::Image::from_icon_name("application-x-executable"),
    };
    icon.set_pixel_size(32);
    icon.add_css_class("lowres-icon");
    row.add_prefix(&icon);

    let suffix = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .build();
    let impact = gtk::Label::builder().label(impact_label(app.impact)).xalign(1.0).build();
    impact.add_css_class(match impact_label(app.impact) {
        "High" => "error",
        "Medium" => "warning",
        _ => "dim-label",
    });
    suffix.append(&impact);
    if let Some(w) = app.watts.filter(|w| *w >= 0.05) {
        suffix.append(&gtk::Label::builder().label(format!("≈ {w:.1} W")).xalign(1.0).css_classes(["caption", "dim-label", "numeric"]).build());
    }
    row.add_suffix(&suffix);
    row
}

fn fill_list(list: &gtk::ListBox, apps: &[AppUsage], show_processes: bool, min_impact: f64) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let shown: Vec<&AppUsage> = apps.iter().filter(|a| a.impact >= min_impact).take(MAX_APPS).collect();
    if shown.is_empty() {
        let row = adw::ActionRow::builder().title("No apps are using significant energy").build();
        row.add_prefix(&gtk::Image::from_icon_name("emblem-ok-symbolic"));
        list.append(&row);
    }
    for app in shown {
        list.append(&app_row(app, show_processes));
    }
}

impl UsagePage {
    pub fn new(ctrl: &Rc<Controller>) -> Rc<Self> {
        let root = adw::PreferencesPage::new();

        // Battery level chart with a range switcher.
        let range = Rc::new(Cell::new(86_400i64));
        let chart_group = adw::PreferencesGroup::builder().title("Battery Level").build();
        let day = gtk::ToggleButton::builder().label("24 Hours").active(true).build();
        let week = gtk::ToggleButton::builder().label("7 Days").group(&day).build();
        let switcher = gtk::Box::builder().css_classes(["linked"]).valign(gtk::Align::Center).build();
        switcher.append(&day);
        switcher.append(&week);
        chart_group.set_header_suffix(Some(&switcher));

        let chart = gtk::DrawingArea::builder().content_height(200).hexpand(true).build();
        let weak: Weak<Controller> = Rc::downgrade(ctrl);
        let r = range.clone();
        chart.set_draw_func(move |area, cr, w, h| {
            let Some(ctrl) = weak.upgrade() else { return };
            let st = ctrl.state();
            let limit = st.battery.as_ref().and_then(|b| b.end_threshold);
            draw_history(cr, w as f64, h as f64, &area.color(), &st.history.samples, now_unix(), r.get(), limit);
        });
        let frame = gtk::Frame::builder().child(&chart).css_classes(["card"]).build();
        chart.set_margin_top(12);
        chart.set_margin_bottom(6);
        chart.set_margin_start(6);
        chart.set_margin_end(12);
        chart_group.add(&frame);
        for (button, secs) in [(&day, 86_400i64), (&week, 7 * 86_400)] {
            let r = range.clone();
            let chart = chart.clone();
            button.connect_toggled(move |b| {
                if b.is_active() {
                    r.set(secs);
                    chart.queue_draw();
                }
            });
        }

        let legend = gtk::Label::builder()
            .label("Green: plugged in · Blue: on battery · Dashed: charge limit")
            .css_classes(["caption", "dim-label"])
            .margin_top(6)
            .xalign(0.0)
            .wrap(true)
            .build();
        chart_group.add(&legend);
        root.add(&chart_group);

        let bg_group = adw::PreferencesGroup::new();
        let background = adw::SwitchRow::builder()
            .title("Record History in the Background")
            .subtitle("History is only recorded while Battery Health Center is running. This starts it hidden when you log in.")
            .active(system::autostart_enabled())
            .build();
        background.connect_active_notify(|row| {
            if let Err(e) = system::set_autostart(row.is_active()) {
                eprintln!("could not update autostart: {e}");
            }
        });
        bg_group.add(&background);
        root.add(&bg_group);

        let now_group = adw::PreferencesGroup::builder()
            .title("Using Significant Energy")
            .description("Apps ranked by their current energy impact.")
            .build();
        let now_list = gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).build();
        now_group.add(&now_list);
        root.add(&now_group);

        let avg_group = adw::PreferencesGroup::builder()
            .title("Average Energy Impact")
            .description("Since Battery Health Center started.")
            .build();
        let avg_list = gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).build();
        avg_group.add(&avg_list);
        root.add(&avg_group);

        let note = adw::PreferencesGroup::new();
        note.add(
            &gtk::Label::builder()
                .label(
                    "Linux does not measure power per app. Energy impact is estimated from CPU time, and watts \
                     are the measured battery drain above idle, shared between apps by CPU use. Treat them as a guide.",
                )
                .css_classes(["caption", "dim-label"])
                .wrap(true)
                .xalign(0.0)
                .build(),
        );
        root.add(&note);

        Rc::new(UsagePage {
            root,
            chart,
            now_group,
            now_list,
            avg_list,
            last_apps_gen: Cell::new(u64::MAX),
            last_history_gen: Cell::new(u64::MAX),
        })
    }

    pub fn update(&self, st: &State) {
        if self.last_history_gen.replace(st.history_generation) != st.history_generation {
            self.chart.queue_draw();
        }
        if self.last_apps_gen.replace(st.apps_generation) == st.apps_generation {
            return;
        }
        let on_battery = st.battery.as_ref().is_some_and(|b| b.is_on_battery());
        self.now_group.set_description(Some(if on_battery {
            "Apps ranked by their current energy impact, with estimated power draw."
        } else {
            "Apps ranked by their current energy impact. Power estimates appear when running on battery."
        }));
        if st.apps_now.is_empty() && st.apps_avg.is_empty() {
            return; // first sample is still being collected
        }
        fill_list(&self.now_list, &st.apps_now, true, 1.0);
        fill_list(&self.avg_list, &st.apps_avg, false, 1.0);
    }
}
