//! Custom-drawn widgets and small row helpers shared by the pages.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use adw::prelude::*;
use gtk::cairo;
use gtk::gdk;
use gtk::glib;

use crate::health_log::HealthEntry;
use crate::history::Sample;
use crate::i18n::tr;

pub const GREEN: (f64, f64, f64) = (0.18, 0.76, 0.49);
pub const YELLOW: (f64, f64, f64) = (0.90, 0.65, 0.04);
pub const RED: (f64, f64, f64) = (0.88, 0.11, 0.14);
pub const BLUE: (f64, f64, f64) = (0.21, 0.52, 0.89);

pub fn level_color(percent: f64, charging: bool) -> (f64, f64, f64) {
    if charging || percent > 50.0 {
        GREEN
    } else if percent > 20.0 {
        YELLOW
    } else {
        RED
    }
}

/// A key/value row: title on the left, dimmed value label on the right.
pub fn value_row(group: &adw::PreferencesGroup, title: &str) -> (adw::ActionRow, gtk::Label) {
    let row = adw::ActionRow::builder().title(title).build();
    let label = gtk::Label::builder().css_classes(["dim-label", "numeric"]).selectable(true).build();
    row.add_suffix(&label);
    group.add(&row);
    (row, label)
}

/// A ring showing the charge level with the percentage in the middle.
pub struct Gauge {
    pub root: gtk::Overlay,
    area: gtk::DrawingArea,
    data: Rc<Cell<(f64, bool)>>,
    percent: gtk::Label,
    caption: gtk::Label,
}

impl Gauge {
    pub fn new() -> Self {
        let data = Rc::new(Cell::new((0.0, false)));
        let area = gtk::DrawingArea::builder().content_width(170).content_height(170).build();
        let d = data.clone();
        area.set_draw_func(move |area, cr, w, h| {
            let (percent, charging) = d.get();
            let fg = area.color();
            let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
            let radius = cx.min(cy) - 10.0;
            cr.set_line_width(12.0);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, 0.1);
            cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
            let _ = cr.stroke();
            if percent > 0.0 {
                let (r, g, b) = level_color(percent, charging);
                cr.set_source_rgb(r, g, b);
                let start = -PI / 2.0;
                cr.arc(cx, cy, radius, start, start + 2.0 * PI * percent / 100.0);
                let _ = cr.stroke();
            }
        });

        let percent = gtk::Label::builder().css_classes(["title-1", "numeric"]).build();
        // Whatever the caption, it must stay inside the ring (about 130 px wide).
        let caption = gtk::Label::builder()
            .css_classes(["caption", "dim-label"])
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(14)
            .build();
        let center = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        center.append(&percent);
        center.append(&caption);

        let root = gtk::Overlay::builder().child(&area).halign(gtk::Align::Center).build();
        root.add_overlay(&center);
        Gauge { root, area, data, percent, caption }
    }

    pub fn set(&self, percent: f64, charging: bool, caption: &str) {
        if self.data.get() != (percent, charging) {
            self.data.set((percent, charging));
            self.area.queue_draw();
        }
        self.percent.set_label(&format!("{percent:.0}%"));
        self.caption.set_label(caption);
    }
}

fn set_rgba(cr: &cairo::Context, c: &gdk::RGBA, alpha: f64) {
    cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, alpha);
}

type Rgb = (f64, f64, f64);

/// What to draw in a time chart.
pub struct ChartSpec<'a> {
    pub start: i64,
    pub end: i64,
    pub y_min: f64,
    pub y_max: f64,
    pub y_ticks: Vec<f64>,
    pub y_label: &'a dyn Fn(f64) -> String,
    /// (timestamp, value, color of the segment ending at this point)
    pub points: &'a [(i64, f64, Rgb)],
    /// Points further apart than this aren't joined (no data in between).
    pub gap: i64,
    pub fill: bool,
    /// Draws a dot at each point (for sparse data).
    pub dots: bool,
    /// Dashed horizontal reference line.
    pub hline: Option<(f64, Rgb)>,
    pub empty_text: &'a str,
}

/// Rounds up to a "nice" chart maximum: 5, 10, 20, 40, 60, ...
pub fn nice_max(v: f64) -> f64 {
    [5.0, 10.0, 20.0, 40.0, 60.0, 80.0, 100.0, 150.0, 200.0].into_iter().find(|m| v <= *m).unwrap_or(v.ceil())
}

pub fn draw_chart(cr: &cairo::Context, width: f64, height: f64, fg: &gdk::RGBA, spec: &ChartSpec) {
    let (left, right, top, bottom) = (44.0, 8.0, 10.0, 24.0);
    let (pw, ph) = (width - left - right, height - top - bottom);
    let range = (spec.end - spec.start).max(1);
    if pw <= 0.0 || ph <= 0.0 {
        return;
    }
    let x_of = |ts: i64| left + (ts - spec.start) as f64 / range as f64 * pw;
    let y_span = (spec.y_max - spec.y_min).max(f64::EPSILON);
    let y_of = |v: f64| top + (1.0 - (v.clamp(spec.y_min, spec.y_max) - spec.y_min) / y_span) * ph;

    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(11.0);
    cr.set_line_width(1.0);
    for &v in &spec.y_ticks {
        let y = y_of(v).round() + 0.5;
        set_rgba(cr, fg, 0.12);
        cr.move_to(left, y);
        cr.line_to(left + pw, y);
        let _ = cr.stroke();
        set_rgba(cr, fg, 0.55);
        let label = (spec.y_label)(v);
        if let Ok(ext) = cr.text_extents(&label) {
            cr.move_to(left - 6.0 - ext.width(), y + ext.height() / 2.0);
            let _ = cr.show_text(&label);
        }
    }

    // Time labels aligned to local hours or days.
    let day = 86_400;
    let (step, fmt) = if range <= 2 * day {
        (6 * 3600, "%H:%M")
    } else if range <= 10 * day {
        (day, "%a")
    } else {
        (((range / day) / 6).max(1) * day, "%b %-d")
    };
    let offset = glib::DateTime::now_local().map(|d| d.utc_offset().as_seconds()).unwrap_or(0);
    let mut t = ((spec.start + offset) / step + 1) * step - offset;
    while t <= spec.end {
        let x = x_of(t).round() + 0.5;
        set_rgba(cr, fg, 0.08);
        cr.move_to(x, top);
        cr.line_to(x, top + ph);
        let _ = cr.stroke();
        if let Ok(label) = glib::DateTime::from_unix_local(t).and_then(|d| d.format(fmt)) {
            set_rgba(cr, fg, 0.55);
            if let Ok(ext) = cr.text_extents(label.trim()) {
                cr.move_to(x - ext.width() / 2.0, height - 6.0);
                let _ = cr.show_text(label.trim());
            }
        }
        t += step;
    }

    if let Some((v, (r, g, b))) = spec.hline {
        let y = y_of(v).round() + 0.5;
        cr.set_source_rgba(r, g, b, 0.7);
        cr.set_dash(&[4.0, 4.0], 0.0);
        cr.move_to(left, y);
        cr.line_to(left + pw, y);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
    }

    let visible: Vec<&(i64, f64, Rgb)> = spec.points.iter().filter(|p| p.0 >= spec.start - spec.gap).collect();
    cr.set_line_width(2.0);
    cr.set_line_join(cairo::LineJoin::Round);
    for pair in visible.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b.0 - a.0 > spec.gap || b.0 < spec.start {
            continue;
        }
        let (r, g, bl) = b.2;
        let (x1, x2) = (x_of(a.0.max(spec.start)), x_of(b.0));
        let (y1, y2) = (y_of(a.1), y_of(b.1));
        if spec.fill {
            cr.set_source_rgba(r, g, bl, 0.15);
            cr.move_to(x1, y1);
            cr.line_to(x2, y2);
            cr.line_to(x2, top + ph);
            cr.line_to(x1, top + ph);
            cr.close_path();
            let _ = cr.fill();
        }
        cr.set_source_rgb(r, g, bl);
        cr.move_to(x1, y1);
        cr.line_to(x2, y2);
        let _ = cr.stroke();
    }
    if spec.dots {
        for p in visible.iter().filter(|p| p.0 >= spec.start) {
            let (r, g, b) = p.2;
            cr.set_source_rgb(r, g, b);
            cr.arc(x_of(p.0), y_of(p.1), 3.0, 0.0, 2.0 * PI);
            let _ = cr.fill();
        }
    }

    if visible.len() < 2 {
        set_rgba(cr, fg, 0.55);
        if let Ok(ext) = cr.text_extents(spec.empty_text) {
            cr.move_to(left + (pw - ext.width()) / 2.0, top + ph / 2.0);
            let _ = cr.show_text(spec.empty_text);
        }
    }
}

/// Samples more than 10 minutes apart mean the app wasn't running.
const HISTORY_GAP: i64 = 600;

/// Battery level: green while plugged in, blue on battery, dashed limit line.
#[allow(clippy::too_many_arguments)]
pub fn draw_level(cr: &cairo::Context, w: f64, h: f64, fg: &gdk::RGBA, samples: &[Sample], now: i64, range: i64, limit: Option<u8>) {
    let points: Vec<_> = samples.iter().map(|s| (s.ts, s.percent, if s.on_power() { GREEN } else { BLUE })).collect();
    draw_chart(cr, w, h, fg, &ChartSpec {
        start: now - range,
        end: now,
        y_min: 0.0,
        y_max: 100.0,
        y_ticks: vec![0.0, 50.0, 100.0],
        y_label: &|v| format!("{v:.0}%"),
        points: &points,
        gap: HISTORY_GAP,
        fill: true,
        dots: false,
        hline: limit.filter(|l| *l < 100).map(|l| (l as f64, GREEN)),
        empty_text: tr("Not enough history yet"),
    });
}

/// Power draw in watts: blue on battery, green while charging.
pub fn draw_power(cr: &cairo::Context, w: f64, h: f64, fg: &gdk::RGBA, samples: &[Sample], now: i64, range: i64) {
    let points: Vec<_> = samples
        .iter()
        .filter(|s| s.ts >= now - range - HISTORY_GAP)
        .filter_map(|s| Some((s.ts, s.power_w?, if s.on_power() { GREEN } else { BLUE })))
        .collect();
    let max = nice_max(points.iter().map(|p| p.1).fold(1.0, f64::max));
    draw_chart(cr, w, h, fg, &ChartSpec {
        start: now - range,
        end: now,
        y_min: 0.0,
        y_max: max,
        y_ticks: vec![0.0, max / 2.0, max],
        y_label: &|v| format!("{v:.0} W"),
        points: &points,
        gap: HISTORY_GAP,
        fill: true,
        dots: false,
        hline: None,
        empty_text: tr("Not enough history yet"),
    });
}

/// Maximum capacity over time from the daily health log, with the 80% line
/// where batteries are usually considered worn.
pub fn draw_health(cr: &cairo::Context, w: f64, h: f64, fg: &gdk::RGBA, entries: &[HealthEntry], now: i64) {
    let points: Vec<_> = entries.iter().map(|e| (e.ts, e.health, BLUE)).collect();
    let start = entries.first().map_or(now - 30 * 86_400, |e| e.ts.min(now - 30 * 86_400));
    let lowest = points.iter().map(|p| p.1).fold(100.0, f64::min);
    let y_min = ((lowest.min(80.0) - 5.0) / 10.0).floor().max(0.0) * 10.0;
    draw_chart(cr, w, h, fg, &ChartSpec {
        start,
        end: now,
        y_min,
        y_max: 100.0,
        y_ticks: vec![y_min, 80.0, 100.0],
        y_label: &|v| format!("{v:.0}%"),
        points: &points,
        gap: i64::MAX,
        fill: false,
        dots: true,
        hline: Some((80.0, YELLOW)),
        empty_text: tr("Recorded once a day; check back tomorrow"),
    });
}
