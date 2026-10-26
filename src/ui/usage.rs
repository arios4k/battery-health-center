use std::cell::Cell;
use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::widgets::{draw_level, draw_power};
use crate::app_history::AppTotal;
use crate::apps::AppUsage;
use crate::battery::format_hours;
use crate::controller::{Controller, State, HIGH_SLEEP_DRAIN};
use crate::history::{now_unix, SleepSession};
use crate::system;

const MAX_APPS: usize = 8;
const MAX_SLEEPS: usize = 5;

#[derive(Clone, Copy, PartialEq)]
enum ChartKind {
    Level,
    Power,
}

/// Range of the "Energy Use by App" list: 0 = since opened, else seconds.
type AppRange = i64;

pub struct UsagePage {
    pub root: adw::PreferencesPage,
    chart: gtk::DrawingArea,
    chart_group: adw::PreferencesGroup,
    legend: gtk::Label,
    sleep_list: gtk::ListBox,
    sleep_mode_row: adw::ActionRow,
    now_group: adw::PreferencesGroup,
    now_list: gtk::ListBox,
    energy_list: gtk::ListBox,
    energy_group: adw::PreferencesGroup,
    app_range: Rc<Cell<AppRange>>,
    last_apps_gen: Cell<u64>,
    last_history_gen: Cell<u64>,
    ctrl: Weak<Controller>,
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

fn app_icon(icon: Option<&gtk::gio::Icon>) -> gtk::Image {
    let image = match icon {
        Some(icon) => gtk::Image::from_gicon(icon),
        None => gtk::Image::from_icon_name("application-x-executable"),
    };
    image.set_pixel_size(32);
    image.add_css_class("lowres-icon");
    image
}

fn suffix_labels(top: &str, top_class: &str, bottom: Option<String>) -> gtk::Box {
    let b = gtk::Box::builder().orientation(gtk::Orientation::Vertical).valign(gtk::Align::Center).build();
    let label = gtk::Label::builder().label(top).xalign(1.0).build();
    label.add_css_class(top_class);
    b.append(&label);
    if let Some(text) = bottom {
        b.append(&gtk::Label::builder().label(text).xalign(1.0).css_classes(["caption", "dim-label", "numeric"]).build());
    }
    b
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
    let row = adw::ActionRow::builder().title(&app.name).subtitle(subtitle.join(" · ")).use_markup(false).build();
    row.add_prefix(&app_icon(app.icon.as_ref()));
    let label = impact_label(app.impact);
    let class = match label {
        "High" => "error",
        "Medium" => "warning",
        _ => "dim-label",
    };
    let watts = app.watts.filter(|w| *w >= 0.05).map(|w| format!("≈ {w:.1} W"));
    row.add_suffix(&suffix_labels(label, class, watts));
    row
}

fn clear(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn placeholder(list: &gtk::ListBox, icon: &str, title: &str, subtitle: Option<&str>) {
    let row = adw::ActionRow::builder().title(title).build();
    if let Some(s) = subtitle {
        row.set_subtitle(s);
    }
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    list.append(&row);
}

fn fill_now(list: &gtk::ListBox, apps: &[AppUsage]) {
    clear(list);
    let shown: Vec<&AppUsage> = apps.iter().filter(|a| a.impact >= 1.0).take(MAX_APPS).collect();
    if shown.is_empty() {
        placeholder(list, "emblem-ok-symbolic", "No apps are using significant energy", None);
    }
    for app in shown {
        list.append(&app_row(app, true));
    }
}

/// Rows for stored totals: share of all app activity, and Wh on battery.
fn fill_totals(list: &gtk::ListBox, totals: &[AppTotal], ctrl: &Controller) {
    clear(list);
    let all: f64 = totals.iter().map(|t| t.cpu_secs).sum();
    let shown: Vec<&AppTotal> = totals.iter().filter(|t| all > 0.0 && t.cpu_secs / all >= 0.01).take(MAX_APPS).collect();
    if shown.is_empty() {
        placeholder(list, "dialog-information-symbolic", "No app activity recorded yet", Some("Usage is recorded while Battery Health Center is running."));
    }
    for t in shown {
        let share = t.cpu_secs / all * 100.0;
        let row = adw::ActionRow::builder()
            .title(&t.name)
            .subtitle(format!("{} of CPU time", format_hours(t.cpu_secs / 3600.0)))
            .use_markup(false)
            .build();
        row.add_prefix(&app_icon(ctrl.app_icon(&t.key).as_ref()));
        let wh = (t.wh >= 0.01).then(|| format!("≈ {:.1} Wh on battery", t.wh));
        row.add_suffix(&suffix_labels(&format!("{share:.0}%"), "numeric", wh));
        list.append(&row);
    }
}

fn sleep_row(s: &SleepSession) -> adw::ActionRow {
    let when = gtk::glib::DateTime::from_unix_local(s.start_ts)
        .and_then(|d| d.format("%a %H:%M"))
        .map(|t| t.to_string())
        .unwrap_or_default();
    let used = (s.from_percent - s.to_percent).max(0.0);
    let rate = s.percent_per_hour();
    let row = adw::ActionRow::builder()
        .title(format!("{when} · {} asleep", format_hours(s.slept_secs as f64 / 3600.0)))
        .subtitle(format!("Used {used:.0}% ({rate:.1}% per hour)"))
        .build();
    row.add_prefix(&gtk::Image::from_icon_name("weather-clear-night-symbolic"));
    let (label, class) = if rate >= HIGH_SLEEP_DRAIN { ("High", "error") } else if rate >= 1.0 { ("Moderate", "warning") } else { ("Normal", "success") };
    row.add_suffix(&suffix_labels(label, class, None));
    row
}

fn toggle_group(labels: &[&str]) -> (gtk::Box, Vec<gtk::ToggleButton>) {
    let b = gtk::Box::builder().css_classes(["linked"]).valign(gtk::Align::Center).build();
    let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        let t = gtk::ToggleButton::builder().label(*label).active(i == 0).build();
        if let Some(first) = buttons.first() {
            t.set_group(Some(first));
        }
        b.append(&t);
        buttons.push(t);
    }
    (b, buttons)
}

fn list() -> gtk::ListBox {
    gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).build()
}

impl UsagePage {
    pub fn new(ctrl: &Rc<Controller>) -> Rc<Self> {
        let root = adw::PreferencesPage::new();

        // Chart: battery level or power draw, over 24 hours or 7 days.
        let range = Rc::new(Cell::new(86_400i64));
        let kind = Rc::new(Cell::new(ChartKind::Level));
        let chart_group = adw::PreferencesGroup::builder().title("Battery Level").build();
        let (kind_box, kind_buttons) = toggle_group(&["Level", "Power"]);
        let (range_box, range_buttons) = toggle_group(&["24 Hours", "7 Days"]);
        // Controls go in their own row so the title isn't squeezed on narrow windows.
        let controls = gtk::Box::builder().spacing(6).margin_bottom(12).build();
        controls.append(&kind_box);
        controls.append(&gtk::Box::builder().hexpand(true).build());
        controls.append(&range_box);
        chart_group.add(&controls);

        let chart = gtk::DrawingArea::builder().content_height(200).hexpand(true).build();
        let weak: Weak<Controller> = Rc::downgrade(ctrl);
        let (r, k) = (range.clone(), kind.clone());
        chart.set_draw_func(move |area, cr, w, h| {
            let Some(ctrl) = weak.upgrade() else { return };
            let st = ctrl.state();
            let samples = &st.history.samples;
            match k.get() {
                ChartKind::Level => {
                    let limit = st.battery.as_ref().and_then(|b| b.end_threshold);
                    draw_level(cr, w as f64, h as f64, &area.color(), samples, now_unix(), r.get(), limit)
                }
                ChartKind::Power => draw_power(cr, w as f64, h as f64, &area.color(), samples, now_unix(), r.get()),
            }
        });
        let frame = gtk::Frame::builder().child(&chart).css_classes(["card"]).build();
        chart.set_margin_top(12);
        chart.set_margin_bottom(6);
        chart.set_margin_start(6);
        chart.set_margin_end(12);
        chart_group.add(&frame);
        let legend = gtk::Label::builder()
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
            .subtitle("History, sleep drain and notifications only work while Battery Health Center is running. This starts it hidden when you log in.")
            .active(system::autostart_enabled())
            .build();
        background.connect_active_notify(|row| {
            if let Err(e) = system::set_autostart(row.is_active()) {
                eprintln!("could not update autostart: {e}");
            }
        });
        bg_group.add(&background);
        root.add(&bg_group);

        let sleep_group = adw::PreferencesGroup::builder()
            .title("Battery Use During Sleep")
            .description("How much charge your laptop used while suspended.")
            .build();
        let sleep_mode_row = adw::ActionRow::builder().title("Sleep Mode").build();
        sleep_mode_row.add_prefix(&gtk::Image::from_icon_name("power-profile-power-saver-symbolic"));
        let sleep_list = list();
        sleep_group.add(&sleep_list);
        let mode_list = list();
        mode_list.set_margin_top(12);
        mode_list.append(&sleep_mode_row);
        sleep_group.add(&mode_list);
        root.add(&sleep_group);

        let now_group = adw::PreferencesGroup::builder().title("Using Significant Energy").build();
        let now_list = list();
        now_group.add(&now_list);
        root.add(&now_group);

        let app_range = Rc::new(Cell::new(0));
        let energy_group = adw::PreferencesGroup::builder().title("Energy Use by App").build();
        let (energy_box, energy_buttons) = toggle_group(&["Since Opened", "24 Hours", "7 Days"]);
        energy_box.set_halign(gtk::Align::Start);
        energy_box.set_margin_bottom(12);
        energy_group.add(&energy_box);
        let energy_list = list();
        energy_group.add(&energy_list);
        root.add(&energy_group);

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

        let page = Rc::new(UsagePage {
            root,
            chart,
            chart_group,
            legend,
            sleep_list,
            sleep_mode_row,
            now_group,
            now_list,
            energy_list,
            energy_group,
            app_range,
            last_apps_gen: Cell::new(u64::MAX),
            last_history_gen: Cell::new(u64::MAX),
            ctrl: Rc::downgrade(ctrl),
        });
        page.update_chart_labels(ChartKind::Level);

        for (button, value) in kind_buttons.iter().zip([ChartKind::Level, ChartKind::Power]) {
            let (k, page) = (kind.clone(), Rc::downgrade(&page));
            button.connect_toggled(move |b| {
                if let (true, Some(page)) = (b.is_active(), page.upgrade()) {
                    k.set(value);
                    page.update_chart_labels(value);
                    page.chart.queue_draw();
                }
            });
        }
        for (button, secs) in range_buttons.iter().zip([86_400i64, 7 * 86_400]) {
            let (r, chart) = (range.clone(), page.chart.clone());
            button.connect_toggled(move |b| {
                if b.is_active() {
                    r.set(secs);
                    chart.queue_draw();
                }
            });
        }
        for (button, secs) in energy_buttons.iter().zip([0i64, 86_400, 7 * 86_400]) {
            let page = Rc::downgrade(&page);
            button.connect_toggled(move |b| {
                if let (true, Some(page)) = (b.is_active(), page.upgrade()) {
                    page.app_range.set(secs);
                    if let Some(ctrl) = page.ctrl.upgrade() {
                        page.fill_energy(&ctrl.state(), &ctrl);
                    }
                }
            });
        }
        page
    }

    fn update_chart_labels(&self, kind: ChartKind) {
        let (title, legend) = match kind {
            ChartKind::Level => ("Battery Level", "Green: plugged in · Blue: on battery · Dashed: charge limit"),
            ChartKind::Power => ("Power Draw", "Blue: power used on battery · Green: charging power"),
        };
        self.chart_group.set_title(title);
        self.legend.set_label(legend);
    }

    fn fill_energy(&self, st: &State, ctrl: &Controller) {
        match self.app_range.get() {
            0 => {
                self.energy_group.set_description(Some("Average energy impact since Battery Health Center started."));
                clear(&self.energy_list);
                let shown: Vec<&AppUsage> = st.apps_avg.iter().filter(|a| a.impact >= 1.0).take(MAX_APPS).collect();
                if shown.is_empty() {
                    placeholder(&self.energy_list, "emblem-ok-symbolic", "No apps are using significant energy", None);
                }
                for app in shown {
                    self.energy_list.append(&app_row(app, false));
                }
            }
            secs => {
                self.energy_group.set_description(Some("Share of app activity, with energy used while on battery."));
                fill_totals(&self.energy_list, &st.app_history.totals(now_unix() - secs), ctrl);
            }
        }
    }

    fn fill_sleep(&self, st: &State) {
        clear(&self.sleep_list);
        let sessions = st.history.sleep_sessions();
        if sessions.is_empty() {
            placeholder(
                &self.sleep_list,
                "weather-clear-night-symbolic",
                "No sleep recorded yet",
                Some("Measured when Battery Health Center is running before and after your laptop sleeps on battery."),
            );
        }
        for s in sessions.iter().take(MAX_SLEEPS) {
            self.sleep_list.append(&sleep_row(s));
        }

        match &st.sleep_mode {
            Some((mode, modes)) => {
                let (name, about) = match mode.as_str() {
                    "s2idle" => ("Modern Standby (s2idle)", "Wakes quickly, but some laptops drain more while asleep"),
                    "deep" => ("Deep Sleep (S3)", "The lowest-power sleep mode"),
                    "shallow" => ("Shallow Sleep (S1)", "A light sleep mode"),
                    other => (other, ""),
                };
                self.sleep_mode_row.set_title(&format!("Sleep Mode: {name}"));
                let high = sessions.iter().take(MAX_SLEEPS).any(|s| s.percent_per_hour() >= HIGH_SLEEP_DRAIN);
                let hint = if mode == "s2idle" && modes.iter().any(|m| m == "deep") && high {
                    format!("{about}. Your laptop also supports deep sleep, which usually uses less battery; you can choose it with the kernel option mem_sleep_default=deep.")
                } else {
                    about.to_string()
                };
                self.sleep_mode_row.set_subtitle(&hint);
                self.sleep_mode_row.set_visible(true);
            }
            None => self.sleep_mode_row.set_visible(false),
        }
    }

    /// Scrolls so the app lists are at the top (used for screenshots).
    pub fn scroll_to_apps(&self) {
        let Some(scrolled) = self.root.first_child().and_downcast::<gtk::ScrolledWindow>() else { return };
        let Some(viewport) = scrolled.child() else { return };
        if let Some(p) = self.now_group.compute_point(&viewport, &gtk::graphene::Point::zero()) {
            scrolled.vadjustment().set_value(p.y() as f64 - 12.0);
        }
    }

    pub fn update(&self, st: &State) {
        if self.last_history_gen.replace(st.history_generation) != st.history_generation {
            self.chart.queue_draw();
            self.fill_sleep(st);
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
        fill_now(&self.now_list, &st.apps_now);
        if let Some(ctrl) = self.ctrl.upgrade() {
            self.fill_energy(st, &ctrl);
        }
    }
}
