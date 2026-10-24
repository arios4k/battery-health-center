//! Custom-drawn widgets and small row helpers shared by the pages.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use adw::prelude::*;
use gtk::cairo;
use gtk::gdk;
use gtk::glib;

use crate::history::Sample;

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
        let caption = gtk::Label::builder().css_classes(["caption", "dim-label"]).build();
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

#[allow(clippy::too_many_arguments)]
/// Draws battery level history: a line per contiguous run of samples, green
/// while plugged in and blue on battery, with an optional charge-limit line.
pub fn draw_history(
    cr: &cairo::Context,
    width: f64,
    height: f64,
    fg: &gdk::RGBA,
    samples: &[Sample],
    now: i64,
    range_secs: i64,
    limit: Option<u8>,
) {
    let (left, right, top, bottom) = (40.0, 8.0, 10.0, 24.0);
    let (pw, ph) = (width - left - right, height - top - bottom);
    if pw <= 0.0 || ph <= 0.0 {
        return;
    }
    let start = now - range_secs;
    let x_of = |ts: i64| left + (ts - start) as f64 / range_secs as f64 * pw;
    let y_of = |pct: f64| top + (1.0 - pct / 100.0) * ph;

    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(11.0);

    // Horizontal grid with percentage labels.
    cr.set_line_width(1.0);
    for pct in [0.0, 50.0, 100.0] {
        let y = y_of(pct).round() + 0.5;
        set_rgba(cr, fg, 0.12);
        cr.move_to(left, y);
        cr.line_to(left + pw, y);
        let _ = cr.stroke();
        set_rgba(cr, fg, 0.55);
        let label = format!("{pct:.0}%");
        if let Ok(ext) = cr.text_extents(&label) {
            cr.move_to(left - 6.0 - ext.width(), y + ext.height() / 2.0);
            let _ = cr.show_text(&label);
        }
    }

    // Time labels along the bottom.
    let (step, fmt) = if range_secs <= 86_400 { (6 * 3600, "%H:%M") } else { (86_400, "%a") };
    let first_tick = {
        // Align ticks to local hour/day boundaries.
        let offset = glib::DateTime::now_local().map(|d| d.utc_offset().as_seconds()).unwrap_or(0);
        ((start + offset) / step + 1) * step - offset
    };
    let mut t = first_tick;
    while t <= now {
        let x = x_of(t).round() + 0.5;
        set_rgba(cr, fg, 0.08);
        cr.move_to(x, top);
        cr.line_to(x, top + ph);
        let _ = cr.stroke();
        if let Ok(label) = glib::DateTime::from_unix_local(t).and_then(|d| d.format(fmt)) {
            set_rgba(cr, fg, 0.55);
            if let Ok(ext) = cr.text_extents(&label) {
                cr.move_to(x - ext.width() / 2.0, height - 6.0);
                let _ = cr.show_text(&label);
            }
        }
        t += step;
    }

    if let Some(limit) = limit.filter(|l| *l < 100) {
        let y = y_of(limit as f64).round() + 0.5;
        cr.set_source_rgba(GREEN.0, GREEN.1, GREEN.2, 0.6);
        cr.set_dash(&[4.0, 4.0], 0.0);
        cr.move_to(left, y);
        cr.line_to(left + pw, y);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
    }

    // Samples more than 10 minutes apart mean the app wasn't running.
    const GAP: i64 = 600;
    let visible: Vec<&Sample> = samples.iter().filter(|s| s.ts >= start - GAP).collect();
    cr.set_line_width(2.0);
    cr.set_line_join(cairo::LineJoin::Round);
    for pair in visible.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b.ts - a.ts > GAP || b.ts < start {
            continue;
        }
        let (r, g, bl) = if b.on_power() { GREEN } else { BLUE };
        let (x1, x2) = (x_of(a.ts.max(start)), x_of(b.ts));
        let (y1, y2) = (y_of(a.percent), y_of(b.percent));
        // Soft fill under the line.
        cr.set_source_rgba(r, g, bl, 0.15);
        cr.move_to(x1, y1);
        cr.line_to(x2, y2);
        cr.line_to(x2, top + ph);
        cr.line_to(x1, top + ph);
        cr.close_path();
        let _ = cr.fill();
        cr.set_source_rgb(r, g, bl);
        cr.move_to(x1, y1);
        cr.line_to(x2, y2);
        let _ = cr.stroke();
    }

    if visible.len() < 2 {
        set_rgba(cr, fg, 0.55);
        let msg = "Not enough history yet";
        if let Ok(ext) = cr.text_extents(msg) {
            cr.move_to(left + (pw - ext.width()) / 2.0, top + ph / 2.0);
            let _ = cr.show_text(msg);
        }
    }
}
