use adw::prelude::*;

use super::widgets::value_row;
use crate::battery::Condition;
use crate::controller::State;
use crate::history::now_unix;

pub struct HealthPage {
    pub root: adw::PreferencesPage,
    capacity: gtk::Label,
    capacity_bar: gtk::LevelBar,
    condition_row: adw::ActionRow,
    condition: gtk::Label,
    full: gtk::Label,
    design: gtk::Label,
    lost: gtk::Label,
    trend_row: adw::ActionRow,
    trend: gtk::Label,
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
}

fn wh(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.1} Wh")).unwrap_or_else(|| "Unknown".into())
}

impl HealthPage {
    pub fn new() -> Self {
        let root = adw::PreferencesPage::new();

        let cap = adw::PreferencesGroup::builder()
            .title("Capacity")
            .description("Batteries lose capacity as they age. Maximum capacity compares how much energy the battery holds now with how much it held when new.")
            .build();
        let (cap_row, capacity) = value_row(&cap, "Maximum Capacity");
        let capacity_bar = gtk::LevelBar::builder()
            .min_value(0.0)
            .max_value(100.0)
            .width_request(140)
            .valign(gtk::Align::Center)
            .build();
        capacity_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_LOW, 60.0);
        capacity_bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_HIGH, 80.0);
        cap_row.add_suffix(&capacity_bar);
        let (condition_row, condition) = value_row(&cap, "Condition");
        let (_, full) = value_row(&cap, "Full Charge Capacity");
        let (_, design) = value_row(&cap, "Design Capacity");
        let (_, lost) = value_row(&cap, "Capacity Lost");
        let (trend_row, trend) = value_row(&cap, "Change Over Last 30 Days");
        root.add(&cap);

        let usage = adw::PreferencesGroup::builder().title("Usage").build();
        let (cycles_row, cycles) = value_row(&usage, "Cycle Count");
        cycles_row.set_subtitle("Most laptop batteries are rated for 300–1000 cycles");
        let (_, energy_now) = value_row(&usage, "Current Charge");
        let (_, avg_draw) = value_row(&usage, "Average Draw on Battery (24 h)");
        let (_, voltage) = value_row(&usage, "Voltage");
        let (_, voltage_design) = value_row(&usage, "Minimum Design Voltage");
        root.add(&usage);

        let info = adw::PreferencesGroup::builder().title("Battery Information").build();
        let (_, manufacturer) = value_row(&info, "Manufacturer");
        let (_, model) = value_row(&info, "Model");
        let (_, serial) = value_row(&info, "Serial Number");
        let (_, technology) = value_row(&info, "Chemistry");
        let (_, device) = value_row(&info, "Device");
        root.add(&info);

        HealthPage {
            root,
            capacity,
            capacity_bar,
            condition_row,
            condition,
            full,
            design,
            lost,
            trend_row,
            trend,
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
        }
    }

    pub fn update(&self, st: &State) {
        let Some(b) = &st.battery else { return };
        let unknown = || "Unknown".to_string();

        let health = b.health_percent();
        self.capacity.set_label(&health.map(|h| format!("{h:.1}%")).unwrap_or_else(unknown));
        self.capacity_bar.set_value(health.unwrap_or(0.0));
        self.capacity_bar.set_visible(health.is_some());

        let cond = b.condition();
        self.condition.set_label(cond.map(Condition::label).unwrap_or("Unknown"));
        self.condition_row.set_subtitle(match cond {
            Some(Condition::Normal) => "The battery is functioning normally",
            Some(Condition::Worn) => "The battery holds less charge than when new but works normally",
            Some(Condition::ServiceRecommended) => "The battery holds significantly less charge; consider replacing it",
            None => "",
        });

        self.full.set_label(&wh(b.energy_full_wh));
        self.design.set_label(&wh(b.energy_design_wh));
        let lost = match (b.energy_full_wh, b.energy_design_wh) {
            (Some(f), Some(d)) if d > f => format!("{:.1} Wh ({:.1}%)", d - f, (d - f) / d * 100.0),
            (Some(_), Some(_)) => "None".into(),
            _ => unknown(),
        };
        self.lost.set_label(&lost);

        // Compare against the oldest recorded health within the last 30 days.
        let oldest = st
            .history
            .samples
            .iter()
            .find(|s| s.ts >= now_unix() - 30 * 86_400 && s.health.is_some())
            .filter(|s| now_unix() - s.ts >= 86_400);
        match (oldest.and_then(|s| s.health), health) {
            (Some(then), Some(now)) => {
                self.trend.set_label(&format!("{:+.1} points", now - then));
                self.trend_row.set_subtitle("Short-term changes are normal; calibration varies");
            }
            _ => {
                self.trend.set_label("Collecting data");
                self.trend_row.set_subtitle("Available after a day of history");
            }
        }

        self.cycles.set_label(&b.cycle_count.map(|c| c.to_string()).unwrap_or_else(|| "Not reported".into()));
        self.energy_now.set_label(&wh(b.energy_now_wh));
        self.avg_draw.set_label(
            &st.history
                .average_discharge_w(now_unix() - 86_400)
                .map(|w| format!("{w:.1} W"))
                .unwrap_or_else(|| "No data yet".into()),
        );
        self.voltage.set_label(&b.voltage_v.map(|v| format!("{v:.2} V")).unwrap_or_else(unknown));
        self.voltage_design.set_label(&b.voltage_design_v.map(|v| format!("{v:.2} V")).unwrap_or_else(unknown));
        self.manufacturer.set_label(b.manufacturer.as_deref().unwrap_or("Unknown"));
        self.model.set_label(b.model.as_deref().unwrap_or("Unknown"));
        self.serial.set_label(b.serial.as_deref().unwrap_or("Unknown"));
        self.technology.set_label(b.technology.as_deref().unwrap_or("Unknown"));
        self.device.set_label(&b.name);
    }
}
