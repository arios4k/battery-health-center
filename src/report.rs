//! A self-contained HTML battery report (like Windows' powercfg
//! /batteryreport), for warranty claims, selling a laptop, or keeping records.
//! Charts are inline SVG; there are no external resources.

use std::fmt::Write;
use std::fs;

use crate::battery::{format_hours, Condition};
use crate::controller::State;
use crate::history::now_unix;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn date(ts: i64, fmt: &str) -> String {
    gtk::glib::DateTime::from_unix_local(ts).and_then(|d| d.format(fmt)).map(|s| s.to_string()).unwrap_or_default()
}

fn dmi(field: &str) -> Option<String> {
    let v = fs::read_to_string(format!("/sys/class/dmi/id/{field}")).ok()?;
    let v = v.trim();
    (!v.is_empty() && v != "None").then(|| v.to_string())
}

/// A small line chart: x is time, y is clamped to [y_min, y_max].
fn svg_chart(points: &[(i64, f64)], start: i64, end: i64, y_min: f64, y_max: f64, unit: &str, dots: bool) -> String {
    let (w, h, left, bottom) = (720.0, 200.0, 48.0, 22.0);
    let (pw, ph) = (w - left - 8.0, h - bottom - 8.0);
    let x = |ts: i64| left + (ts - start) as f64 / (end - start).max(1) as f64 * pw;
    let y = |v: f64| 8.0 + (1.0 - (v.clamp(y_min, y_max) - y_min) / (y_max - y_min).max(1e-9)) * ph;
    let mut svg = format!(r#"<svg viewBox="0 0 {w} {h}" role="img" class="chart">"#);
    for v in [y_min, (y_min + y_max) / 2.0, y_max] {
        let yy = y(v);
        let _ = write!(svg, r#"<line x1="{left}" x2="{}" y1="{yy}" y2="{yy}" class="grid"/><text x="{}" y="{}" class="axis" text-anchor="end">{v:.0}{unit}</text>"#, left + pw, left - 6.0, yy + 4.0);
    }
    let _ = write!(svg, r#"<text x="{left}" y="{}" class="axis">{}</text><text x="{}" y="{}" class="axis" text-anchor="end">{}</text>"#,
        h - 4.0, date(start, "%b %-d"), left + pw, h - 4.0, date(end, "%b %-d"));
    // Break the line where samples are far apart.
    let gap = ((end - start) / 50).max(600);
    let mut path = String::new();
    let mut prev: Option<i64> = None;
    for &(ts, v) in points.iter().filter(|p| p.0 >= start) {
        let cmd = if prev.is_some_and(|p| ts - p <= gap) { 'L' } else { 'M' };
        let _ = write!(path, "{cmd}{:.1},{:.1} ", x(ts), y(v));
        prev = Some(ts);
    }
    let _ = write!(svg, r#"<path d="{path}" class="line"/>"#);
    if dots {
        for &(ts, v) in points.iter().filter(|p| p.0 >= start) {
            let _ = write!(svg, r#"<circle cx="{:.1}" cy="{:.1}" r="3" class="dot"/>"#, x(ts), y(v));
        }
    }
    svg.push_str("</svg>");
    svg
}

fn row(out: &mut String, k: &str, v: &str) {
    let _ = write!(out, "<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(v));
}

pub fn build_html(st: &State) -> String {
    let now = now_unix();
    let wh = |v: Option<f64>| v.map(|v| format!("{v:.1} Wh")).unwrap_or_else(|| "Unknown".into());
    let mut b = String::new();
    let computer = [dmi("sys_vendor"), dmi("product_version").or_else(|| dmi("product_name"))]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");

    let _ = write!(b, "<h1>Battery Report</h1><p class=\"sub\">{}{}</p>", esc(&date(now, "%A, %B %-d, %Y at %H:%M")),
        if computer.is_empty() { String::new() } else { format!(" · {}", esc(&computer)) });

    for bat in &st.batteries {
        let _ = write!(b, "<h2>Battery {}</h2><table>", esc(&bat.name));
        row(&mut b, "Manufacturer", bat.manufacturer.as_deref().unwrap_or("Unknown"));
        row(&mut b, "Model", bat.model.as_deref().unwrap_or("Unknown"));
        row(&mut b, "Serial number", bat.serial.as_deref().unwrap_or("Unknown"));
        row(&mut b, "Chemistry", bat.technology.as_deref().unwrap_or("Unknown"));
        row(&mut b, "Design capacity", &wh(bat.energy_design_wh));
        row(&mut b, "Full charge capacity", &wh(bat.energy_full_wh));
        row(&mut b, "Maximum capacity", &bat.health_percent().map(|h| format!("{h:.1}%")).unwrap_or_else(|| "Unknown".into()));
        row(&mut b, "Condition", bat.condition().map(Condition::label).unwrap_or("Unknown"));
        row(&mut b, "Cycle count", &bat.cycle_count.map(|c| c.to_string()).unwrap_or_else(|| "Not reported".into()));
        row(&mut b, "Current charge", &format!("{:.0}% ({})", bat.percent, wh(bat.energy_now_wh)));
        let limit = match (bat.end_threshold.filter(|e| *e < 100), bat.start_threshold.filter(|s| *s > 0)) {
            (Some(e), Some(s)) => format!("Stops at {e}%, resumes below {s}%"),
            (Some(e), None) => format!("Stops at {e}%"),
            (None, _) if bat.supports_end_threshold() => "None (charges to 100%)".into(),
            _ => "Not supported".into(),
        };
        row(&mut b, "Charge limit", &limit);
        b.push_str("</table>");
    }

    b.push_str("<h2>Capacity Over Time</h2>");
    let entries = &st.health_log.entries;
    if entries.len() >= 2 {
        let pts: Vec<(i64, f64)> = entries.iter().map(|e| (e.ts, e.health)).collect();
        let lowest = pts.iter().map(|p| p.1).fold(100.0, f64::min);
        let y_min = ((lowest.min(80.0) - 5.0) / 10.0).floor().max(0.0) * 10.0;
        b.push_str(&svg_chart(&pts, entries[0].ts, now, y_min, 100.0, "%", true));
    }
    if let Some(t) = st.health_log.trend() {
        let _ = write!(b, "<p>Trend: {:+.1}% per month over {:.0} days", t.per_month, t.span_days);
        if let Some(ts) = t.reaches_80_ts {
            let _ = write!(b, "; projected to reach 80% around {}", esc(&date(ts, "%B %Y")));
        }
        b.push_str(".</p>");
    }
    if !entries.is_empty() {
        b.push_str("<table><tr><th>Date</th><th>Maximum capacity</th><th>Cycles</th></tr>");
        for e in entries.iter().rev().take(30) {
            let _ = write!(b, "<tr><td>{}</td><td>{:.1}%</td><td>{}</td></tr>", esc(&date(e.ts, "%Y-%m-%d")), e.health,
                e.cycles.map(|c| c.to_string()).unwrap_or_default());
        }
        b.push_str("</table>");
    } else {
        b.push_str("<p class=\"sub\">No capacity history recorded yet.</p>");
    }

    b.push_str("<h2>Battery Level, Last 7 Days</h2>");
    let week = now - 7 * 86_400;
    let level: Vec<(i64, f64)> = st.history.samples.iter().filter(|s| s.ts >= week).map(|s| (s.ts, s.percent)).collect();
    if level.len() >= 2 {
        b.push_str(&svg_chart(&level, week, now, 0.0, 100.0, "%", false));
    } else {
        b.push_str("<p class=\"sub\">Not enough history recorded yet.</p>");
    }
    b.push_str("<table>");
    for (label, since) in [("Average draw on battery, 24 hours", now - 86_400), ("Average draw on battery, 7 days", week)] {
        row(&mut b, label, &st.history.average_discharge_w(since).map(|w| format!("{w:.1} W")).unwrap_or_else(|| "No data".into()));
    }
    if let Some(s) = st.history.since_last_charge() {
        row(&mut b, "Since last charge", &format!("{} on battery, {:.0}% used",
            format_hours((now - s.unplugged_ts) as f64 / 3600.0), s.used_percent));
    }
    b.push_str("</table>");

    b.push_str("<h2>Battery Use During Sleep</h2>");
    let sleeps = st.history.sleep_sessions();
    if sleeps.is_empty() {
        b.push_str("<p class=\"sub\">No sleep recorded yet.</p>");
    } else {
        b.push_str("<table><tr><th>Went to sleep</th><th>Duration</th><th>Used</th><th>Per hour</th></tr>");
        for s in sleeps.iter().take(10) {
            let _ = write!(b, "<tr><td>{}</td><td>{}</td><td>{:.0}%</td><td>{:.1}%</td></tr>", esc(&date(s.start_ts, "%a %b %-d, %H:%M")),
                format_hours(s.slept_secs as f64 / 3600.0), (s.from_percent - s.to_percent).max(0.0), s.percent_per_hour());
        }
        b.push_str("</table>");
    }
    if let Some((mode, _)) = &st.sleep_mode {
        let _ = write!(b, "<p>Sleep mode: {}</p>", esc(mode));
    }

    b.push_str("<h2>Apps by Energy Use, Last 7 Days</h2>");
    let totals = st.app_history.totals(week);
    let all: f64 = totals.iter().map(|t| t.cpu_secs).sum();
    if all > 0.0 {
        b.push_str("<table><tr><th>App</th><th>Share of activity</th><th>CPU time</th><th>Energy on battery</th></tr>");
        for t in totals.iter().take(15) {
            let _ = write!(b, "<tr><td>{}</td><td>{:.0}%</td><td>{}</td><td>{:.1} Wh</td></tr>", esc(&t.name),
                t.cpu_secs / all * 100.0, format_hours(t.cpu_secs / 3600.0), t.wh);
        }
        b.push_str("</table><p class=\"sub\">Estimated from CPU time; Linux doesn't measure power per app.</p>");
    } else {
        b.push_str("<p class=\"sub\">No app activity recorded yet.</p>");
    }

    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Battery Report</title><style>
:root{{--bg:#fff;--fg:#1e1e1e;--dim:#6e6e6e;--line:#e3e3e3;--accent:#3584e4}}
@media (prefers-color-scheme:dark){{:root{{--bg:#1e1e1e;--fg:#f0f0f0;--dim:#a0a0a0;--line:#3a3a3a;--accent:#78aeed}}}}
body{{background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,sans-serif;max-width:760px;margin:32px auto;padding:0 16px}}
h1{{margin-bottom:0}}h2{{margin-top:36px;border-bottom:1px solid var(--line);padding-bottom:4px}}
.sub{{color:var(--dim)}}table{{border-collapse:collapse;width:100%;margin:12px 0}}
th,td{{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line);font-variant-numeric:tabular-nums}}
th{{font-weight:600;width:40%}}tr th:only-child{{width:auto}}
.chart{{width:100%;height:auto}}.grid{{stroke:var(--line)}}.axis{{fill:var(--dim);font-size:11px}}
.line{{fill:none;stroke:var(--accent);stroke-width:2}}.dot{{fill:var(--accent)}}
@media print{{body{{margin:0}}h2{{break-after:avoid}}}}
</style></head><body>{b}<p class="sub">Generated by Battery Health Center.</p></body></html>"#
    )
}
