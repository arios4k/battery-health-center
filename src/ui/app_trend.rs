//! A window with one app's energy use hour by hour, opened from the Usage page.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;

use super::widgets::{draw_app_trend, TrendKind};
use crate::battery::format_hours;
use crate::controller::Controller;
use crate::history::now_unix;
use crate::i18n::{tr, trf};

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

/// "2 h 10 min of CPU time · ≈ 0.8 Wh on battery".
fn summary(cpu_secs: f64, wh: f64) -> String {
    let cpu = if cpu_secs < 1.0 { tr("No activity").to_string() } else { trf("{0} of CPU time", &[("0", format_hours(cpu_secs / 3600.0))]) };
    if wh >= 0.01 {
        format!("{cpu} · {}", trf("≈ {0} Wh on battery", &[("0", format!("{wh:.1}"))]))
    } else {
        cpu
    }
}

pub fn show(parent: &impl IsA<gtk::Widget>, ctrl: &Rc<Controller>, key: &str, name: &str, icon: Option<gio::Icon>) {
    let now = now_unix();
    let st = ctrl.state();
    // The week's points; shorter ranges are cut from it when drawing.
    let points = Rc::new(st.app_history.series(key, now - 7 * 86_400, now));
    drop(st);
    let totals = |since: i64| {
        points.iter().filter(|p| p.hour + 3600 > since).fold((0.0, 0.0), |(c, w), p| (c + p.cpu_secs, w + p.wh))
    };

    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).margin_top(12).margin_bottom(24).margin_start(18).margin_end(18).build();

    let head = gtk::Box::builder().spacing(12).build();
    let image = match &icon {
        Some(icon) => gtk::Image::from_gicon(icon),
        None => gtk::Image::from_icon_name("application-x-executable"),
    };
    image.set_pixel_size(48);
    head.append(&image);
    head.append(&gtk::Label::builder().label(name).css_classes(["title-2"]).xalign(0.0).wrap(true).hexpand(true).build());
    content.append(&head);

    let group = adw::PreferencesGroup::new();
    for (title, secs) in [(tr("Last 24 Hours"), 86_400), (tr("Last 7 Days"), 7 * 86_400)] {
        let (cpu, wh) = totals(now - secs);
        let row = adw::ActionRow::builder().title(title).subtitle(summary(cpu, wh)).subtitle_selectable(false).build();
        group.add(&row);
    }
    content.append(&group);

    let range = Rc::new(Cell::new(86_400i64));
    let kind = Rc::new(Cell::new(TrendKind::Impact));
    let (kind_box, kind_buttons) = toggle_group(&[tr("Energy Impact"), tr("Energy Used")]);
    let (range_box, range_buttons) = toggle_group(&[tr("24 Hours"), tr("7 Days")]);
    let controls = gtk::Box::builder().spacing(6).build();
    controls.append(&kind_box);
    controls.append(&gtk::Box::builder().hexpand(true).build());
    controls.append(&range_box);
    content.append(&controls);

    let chart = gtk::DrawingArea::builder().content_height(200).hexpand(true).build();
    let (p, r, k) = (points.clone(), range.clone(), kind.clone());
    chart.set_draw_func(move |area, cr, w, h| draw_app_trend(cr, w as f64, h as f64, &area.color(), &p, now_unix(), r.get(), k.get()));
    chart.set_margin_top(12);
    chart.set_margin_bottom(6);
    chart.set_margin_start(6);
    chart.set_margin_end(12);
    content.append(&gtk::Frame::builder().child(&chart).css_classes(["card"]).build());

    let legend = gtk::Label::builder().css_classes(["caption", "dim-label"]).xalign(0.0).wrap(true).build();
    let describe = |k: TrendKind| match k {
        TrendKind::Impact => tr("Average CPU use in each hour, in percent of one core. Only hours when Battery Health Center was running are shown."),
        TrendKind::Energy => tr("Estimated energy drawn from the battery in each hour. Linux doesn't measure power per app, so this is an estimate."),
    };
    legend.set_label(describe(TrendKind::Impact));
    content.append(&legend);

    for (button, value) in kind_buttons.iter().zip([TrendKind::Impact, TrendKind::Energy]) {
        let (k, chart, legend) = (kind.clone(), chart.clone(), legend.clone());
        button.connect_toggled(move |b| {
            if b.is_active() {
                k.set(value);
                legend.set_label(describe(value));
                chart.queue_draw();
            }
        });
    }
    for (button, secs) in range_buttons.iter().zip([86_400i64, 7 * 86_400]) {
        let (r, chart) = (range.clone(), chart.clone());
        button.connect_toggled(move |b| {
            if b.is_active() {
                r.set(secs);
                chart.queue_draw();
            }
        });
    }

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&gtk::ScrolledWindow::builder().child(&content).hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build()));
    let dialog = adw::Dialog::builder().title(name).content_width(520).content_height(620).child(&toolbar).build();
    dialog.present(Some(parent));
}
