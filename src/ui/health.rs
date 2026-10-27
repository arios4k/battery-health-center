use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::widgets::{draw_health, value_row};
use crate::battery::Condition;
use crate::controller::{Controller, State, CALIBRATION_EMPTY};
use crate::history::now_unix;
use crate::settings::Calibration;
use crate::i18n::{tr, trf};

pub struct HealthPage {
    pub root: adw::PreferencesPage,
    battery_group: adw::PreferencesGroup,
    battery_select: adw::ComboRow,
    battery_names: RefCell<Vec<String>>,
    selected: Rc<Cell<usize>>,
    chart: gtk::DrawingArea,
    trend_month: gtk::Label,
    projection_row: adw::ActionRow,
    projection: gtk::Label,
    last_log_len: Cell<usize>,
    calib_row: adw::ActionRow,
    calib_button: gtk::Button,
    calib_step: Cell<Option<Calibration>>,
    capacity: gtk::Label,
    capacity_bar: gtk::LevelBar,
    condition_row: adw::ActionRow,
    condition: gtk::Label,
    full: gtk::Label,
    design: gtk::Label,
    lost: gtk::Label,
    cycles: gtk::Label,
    energy_now: gtk::Label,
    voltage: gtk::Label,
    voltage_design: gtk::Label,
    avg_draw: gtk::Label,
    manufacturer: gtk::Label,
    model: gtk::Label,
    serial: gtk::Label,
    technology: gtk::Label,
    device: gtk::Label,
    ctrl: Weak<Controller>,
}

fn wh(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.1} Wh")).unwrap_or_else(|| tr("Unknown").into())
}

impl HealthPage {
    pub fn new(ctrl: &Rc<Controller>) -> Rc<Self> {
        let root = adw::PreferencesPage::new();

        // Only shown on laptops with more than one battery.
        let battery_group = adw::PreferencesGroup::builder().visible(false).build();
        let battery_select = adw::ComboRow::builder().title(tr("Battery")).build();
        battery_group.add(&battery_select);
        root.add(&battery_group);

        let cap = adw::PreferencesGroup::builder()
            .title(tr("Capacity"))
            .description(tr("Batteries lose capacity as they age. Maximum capacity compares how much energy the battery holds now with how much it held when new."))
            .build();
        let (cap_row, capacity) = value_row(&cap, tr("Maximum Capacity"));
        let capacity_bar = gtk::LevelBar::builder()
            .min_value(0.0)
            .max_value(100.0)
            .width_request(140)
            .valign(gtk::Align::Center)
            .build();
        capacity_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_LOW, 60.0);
        capacity_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_HIGH, 80.0);
        cap_row.add_suffix(&capacity_bar);
        let (condition_row, condition) = value_row(&cap, tr("Condition"));
        let (_, full) = value_row(&cap, tr("Full Charge Capacity"));
        let (_, design) = value_row(&cap, tr("Design Capacity"));
        let (_, lost) = value_row(&cap, tr("Capacity Lost"));
        root.add(&cap);

        let over_time = adw::PreferencesGroup::builder()
            .title(tr("Capacity Over Time"))
            .description(tr("Recorded once a day. Readings vary a little from day to day; the trend shows the overall direction."))
            .build();
        let chart = gtk::DrawingArea::builder().content_height(180).hexpand(true).build();
        let weak = Rc::downgrade(ctrl);
        chart.set_draw_func(move |area, cr, w, h| {
            if let Some(ctrl) = weak.upgrade() {
                draw_health(cr, w as f64, h as f64, &area.color(), &ctrl.state().health_log.entries, now_unix());
            }
        });
        chart.set_margin_top(12);
        chart.set_margin_bottom(6);
        chart.set_margin_start(6);
        chart.set_margin_end(12);
        over_time.add(&gtk::Frame::builder().child(&chart).css_classes(["card"]).build());
        let rows = adw::PreferencesGroup::new();
        let (_, trend_month) = value_row(&rows, tr("Trend"));
        let (projection_row, projection) = value_row(&rows, tr("Reaches 80%"));
        projection_row.set_subtitle(tr("When batteries are usually considered worn"));
        rows.set_margin_top(12);
        over_time.add(&rows);
        root.add(&over_time);

        let usage = adw::PreferencesGroup::builder().title(tr("Usage")).build();
        let (cycles_row, cycles) = value_row(&usage, tr("Cycle Count"));
        cycles_row.set_subtitle(tr("Most laptop batteries are rated for 300–1000 cycles"));
        let (_, energy_now) = value_row(&usage, tr("Current Charge"));
        let (_, avg_draw) = value_row(&usage, tr("Average Draw on Battery (24 h)"));
        let (_, voltage) = value_row(&usage, tr("Voltage"));
        let (_, voltage_design) = value_row(&usage, tr("Minimum Design Voltage"));
        root.add(&usage);

        let info = adw::PreferencesGroup::builder().title(tr("Battery Information")).build();
        let (_, manufacturer) = value_row(&info, tr("Manufacturer"));
        let (_, model) = value_row(&info, tr("Model"));
        let (_, serial) = value_row(&info, tr("Serial Number"));
        let (_, technology) = value_row(&info, tr("Chemistry"));
        let (_, device) = value_row(&info, tr("Device"));
        root.add(&info);

        let calib = adw::PreferencesGroup::builder()
            .title(tr("Calibration"))
            .description(
                tr("If the percentage jumps or the laptop shuts down before reaching 0%, recalibrating helps the battery report its capacity accurately. It takes a full charge, a full discharge and another full charge, usually a few hours."),
            )
            .build();
        let calib_row = adw::ActionRow::builder().title(tr("Not running")).build();
        calib_row.add_prefix(&gtk::Image::from_icon_name("emblem-synchronizing-symbolic"));
        let calib_button = gtk::Button::builder().label(tr("Start")).valign(gtk::Align::Center).build();
        calib_row.add_suffix(&calib_button);
        calib.add(&calib_row);
        root.add(&calib);

        let page = Rc::new(HealthPage {
            root,
            battery_group,
            battery_select,
            battery_names: RefCell::new(Vec::new()),
            selected: Rc::new(Cell::new(0)),
            chart,
            trend_month,
            projection_row,
            projection,
            last_log_len: Cell::new(usize::MAX),
            calib_row,
            calib_button,
            calib_step: Cell::new(None),
            capacity,
            capacity_bar,
            condition_row,
            condition,
            full,
            design,
            lost,
            cycles,
            energy_now,
            voltage,
            voltage_design,
            avg_draw,
            manufacturer,
            model,
            serial,
            technology,
            device,
            ctrl: Rc::downgrade(ctrl),
        });

        let sel = page.selected.clone();
        let weak = Rc::downgrade(&page);
        page.battery_select.connect_selected_notify(move |row| {
            sel.set(row.selected() as usize);
            if let Some(page) = weak.upgrade() {
                if let Some(ctrl) = page.ctrl.upgrade() {
                    page.update(&ctrl.state());
                }
            }
        });
        let weak = Rc::downgrade(&page);
        page.calib_button.connect_clicked(move |button| {
            let Some(page) = weak.upgrade() else { return };
            let Some(ctrl) = page.ctrl.upgrade() else { return };
            if ctrl.state().settings.calibration != Calibration::Off {
                ctrl.cancel_calibration();
                return;
            }
            let dialog = adw::AlertDialog::builder()
                .heading(tr("Start Calibration?"))
                .body(trf("Your laptop will charge to 100%. Then unplug it and use it normally until the battery reaches {CALIBRATION_EMPTY}%, and charge it to 100% again. You'll get a notification at each step, and your charge limit comes back at the end.", &[("CALIBRATION_EMPTY", format!("{CALIBRATION_EMPTY:.0}"))]))
                .build();
            dialog.add_responses(&[("cancel", tr("Cancel")), ("start", tr("Start"))]);
            dialog.set_response_appearance("start", adw::ResponseAppearance::Suggested);
            let weak_ctrl = Rc::downgrade(&ctrl);
            dialog.connect_response(None, move |_, response| {
                if let (Some(ctrl), "start") = (weak_ctrl.upgrade(), response) {
                    ctrl.start_calibration();
                }
            });
            dialog.present(Some(button));
        });
        page
    }

    fn update_calibration(&self, st: &State) {
        let Some(b) = st.battery.as_ref() else { return };
        let step = st.settings.calibration;
        let pct = b.percent;
        let (title, subtitle) = match step {
            Calibration::Off => (tr("Not running").to_string(), tr("You'll get a notification at each step.").to_string()),
            Calibration::ChargeFull | Calibration::Recharge => {
                let n = if step == Calibration::ChargeFull { 1 } else { 3 };
                let sub = if b.ac_online { trf("Charging, now at {pct}%", &[("pct", format!("{pct:.0}"))]) } else { tr("Connect the charger to continue").into() };
                (trf("Step {n} of 3: Charge to 100%", &[("n", n.to_string())]), sub)
            }
            Calibration::Discharge => {
                let sub = if b.ac_online {
                    tr("Unplug the charger to continue").to_string()
                } else {
                    trf("Keep using your laptop until {CALIBRATION_EMPTY}%; now at {pct}%", &[("CALIBRATION_EMPTY", format!("{CALIBRATION_EMPTY:.0}")), ("pct", format!("{pct:.0}"))])
                };
                (tr("Step 2 of 3: Use the battery until nearly empty").to_string(), sub)
            }
        };
        self.calib_row.set_title(&title);
        self.calib_row.set_subtitle(&subtitle);
        if self.calib_step.replace(Some(step)) != Some(step) {
            let running = step != Calibration::Off;
            self.calib_button.set_label(if running { tr("Cancel") } else { tr("Start") });
            if running {
                self.calib_button.remove_css_class("suggested-action");
            } else {
                self.calib_button.add_css_class("suggested-action");
            }
        }
    }

    fn update_trend(&self, st: &State) {
        if self.last_log_len.replace(st.health_log.entries.len()) != st.health_log.entries.len() {
            self.chart.queue_draw();
        }
        match st.health_log.trend() {
            Some(t) => {
                self.trend_month.set_label(&trf("{0}% per month", &[("0", format!("{:+.1}", t.per_month))]));
                let when = t.reaches_80_ts.and_then(|ts| gtk::glib::DateTime::from_unix_local(ts).ok()?.format("%B %Y").ok());
                self.projection_row.set_visible(true);
                self.projection.set_label(&match (when, t.per_month >= 0.0) {
                    (Some(w), _) => trf("Around {w}", &[("w", w.to_string())]),
                    (None, true) => tr("Not declining").to_string(),
                    (None, false) => tr("Already below 80%").to_string(),
                });
            }
            None => {
                let days = st.health_log.entries.first().map_or(0, |e| (now_unix() - e.ts) / 86_400);
                self.trend_month.set_label(&trf("Collecting data ({days} of 14 days)", &[("days", days.to_string())]));
                self.projection_row.set_visible(false);
            }
        }
    }

    pub fn update(&self, st: &State) {
        self.update_calibration(st);
        self.update_trend(st);

        let names: Vec<String> = st.batteries.iter().map(|b| b.name.clone()).collect();
        self.battery_group.set_visible(names.len() > 1);
        if *self.battery_names.borrow() != names {
            *self.battery_names.borrow_mut() = names.clone();
            let labels: Vec<&str> = names.iter().map(String::as_str).collect();
            self.battery_select.set_model(Some(&gtk::StringList::new(&labels)));
        }
        let Some(b) = st.batteries.get(self.selected.get()).or(st.batteries.first()) else { return };
        let unknown = || tr("Unknown").to_string();

        let health = b.health_percent();
        self.capacity.set_label(&health.map(|h| format!("{h:.1}%")).unwrap_or_else(unknown));
        self.capacity_bar.set_value(health.unwrap_or(0.0));
        self.capacity_bar.set_visible(health.is_some());

        let cond = b.condition();
        self.condition.set_label(cond.map(Condition::label).unwrap_or(tr("Unknown")));
        self.condition_row.set_subtitle(match cond {
            Some(Condition::Normal) => tr("The battery is functioning normally"),
            Some(Condition::Worn) => tr("The battery holds less charge than when new but works normally"),
            Some(Condition::ServiceRecommended) => tr("The battery holds significantly less charge; consider replacing it"),
            None => "",
        });

        self.full.set_label(&wh(b.energy_full_wh));
        self.design.set_label(&wh(b.energy_design_wh));
        let lost = match (b.energy_full_wh, b.energy_design_wh) {
            (Some(f), Some(d)) if d > f => format!("{:.1} Wh ({:.1}%)", d - f, (d - f) / d * 100.0),
            (Some(_), Some(_)) => tr("None").into(),
            _ => unknown(),
        };
        self.lost.set_label(&lost);

        self.cycles.set_label(&b.cycle_count.map(|c| c.to_string()).unwrap_or_else(|| tr("Not reported").into()));
        self.energy_now.set_label(&wh(b.energy_now_wh));
        self.avg_draw.set_label(
            &st.history
                .average_discharge_w(now_unix() - 86_400)
                .map(|w| format!("{w:.1} W"))
                .unwrap_or_else(|| tr("No data yet").into()),
        );
        self.voltage.set_label(&b.voltage_v.map(|v| format!("{v:.2} V")).unwrap_or_else(unknown));
        self.voltage_design.set_label(&b.voltage_design_v.map(|v| format!("{v:.2} V")).unwrap_or_else(unknown));
        self.manufacturer.set_label(b.manufacturer.as_deref().unwrap_or(tr("Unknown")));
        self.model.set_label(b.model.as_deref().unwrap_or(tr("Unknown")));
        self.serial.set_label(b.serial.as_deref().unwrap_or(tr("Unknown")));
        self.technology.set_label(b.technology.as_deref().unwrap_or(tr("Unknown")));
        self.device.set_label(&b.name);
    }
}
