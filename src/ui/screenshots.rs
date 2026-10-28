//! Renders each page to a PNG for store listings. Enabled by running the app
//! with BHC_SCREENSHOTS=<folder> (and optionally BHC_SCREENSHOTS_STYLE=light or
//! dark, BHC_SCREENSHOTS_SIZE=<w>x<h>, and BHC_SCREENSHOTS_SCROLL=<pixels> to scroll each
//! page down first); it saves one image per page and quits.

use std::path::PathBuf;
use std::time::Duration;

use adw::prelude::*;
use gtk::{glib, gsk};

const PAGES: &[&str] = &["overview", "health", "charging", "power", "usage"];

pub fn capture(window: &adw::ApplicationWindow, stack: &adw::ViewStack, dir: PathBuf, scroll_usage: impl Fn() + 'static) {
    let _ = std::fs::create_dir_all(&dir);
    // BHC_SCREENSHOTS_SIZE=<width>x<height> overrides the window size.
    if let Some((w, h)) = std::env::var("BHC_SCREENSHOTS_SIZE").ok().and_then(|v| {
        let (w, h) = v.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    }) {
        window.set_default_size(w, h);
    }
    // BHC_SCREENSHOTS_STYLE=light|dark forces the color scheme.
    let scheme = match std::env::var("BHC_SCREENSHOTS_STYLE").as_deref() {
        Ok("light") => adw::ColorScheme::ForceLight,
        Ok("dark") => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
    let window = window.clone();
    let stack = stack.clone();
    // Wait for app usage to be sampled before the first capture.
    glib::timeout_add_local_once(Duration::from_secs(6), move || {
        capture_page(window, stack, dir, Box::new(scroll_usage), 0);
    });
}

fn capture_page(window: adw::ApplicationWindow, stack: adw::ViewStack, dir: PathBuf, scroll_usage: Box<dyn Fn()>, i: usize) {
    let Some(name) = PAGES.get(i) else {
        if let Some(app) = window.application() {
            app.quit();
        }
        return;
    };
    stack.set_visible_child_name(name);
    if *name == "usage" {
        scroll_usage();
    } else if let Some(pixels) = std::env::var("BHC_SCREENSHOTS_SCROLL").ok().and_then(|v| v.parse::<f64>().ok()) {
        // Scroll once the page has been laid out, shortly before the capture.
        let stack = stack.clone();
        glib::timeout_add_local_once(Duration::from_millis(800), move || {
            let scrolled = stack
                .visible_child()
                .and_then(|page| page.first_child())
                .and_then(|sw| sw.downcast::<gtk::ScrolledWindow>().ok());
            if let Some(sw) = scrolled {
                let adj = sw.vadjustment();
                adj.set_value(pixels.min(adj.upper() - adj.page_size()));
            }
        });
    }
    glib::timeout_add_local_once(Duration::from_millis(1200), move || {
        let path = dir.join(format!("{}-{name}.png", i + 1));
        // A page occasionally renders empty right after switching; retry once.
        let rendered = render(&window).or_else(|_| {
            std::thread::sleep(Duration::from_millis(500));
            render(&window)
        });
        match rendered {
            Ok(texture) => match texture.save_to_png(&path) {
                Ok(()) => eprintln!("saved {}", path.display()),
                Err(e) => eprintln!("could not save {}: {e}", path.display()),
            },
            Err(e) => eprintln!("could not render {name}: {e}"),
        }
        // BHC_SCREENSHOTS_FULL also saves each page's whole scrollable content.
        if std::env::var_os("BHC_SCREENSHOTS_FULL").is_some() {
            let content = stack
                .visible_child()
                .and_then(|page| page.first_child()) // AdwPreferencesPage's scrolled window
                .and_then(|sw| sw.downcast::<gtk::ScrolledWindow>().ok())
                .and_then(|sw| sw.child()) // its viewport
                .and_then(|viewport| viewport.first_child());
            if let Some(content) = content {
                let path = dir.join(format!("{}-{name}-full.png", i + 1));
                match render(&content).and_then(|t| t.save_to_png(&path).map_err(|e| e.to_string())) {
                    Ok(()) => eprintln!("saved {}", path.display()),
                    Err(e) => eprintln!("could not render full {name}: {e}"),
                }
            }
        }
        capture_page(window, stack, dir, scroll_usage, i + 1);
    });
}

fn render(widget: &impl IsA<gtk::Widget>) -> Result<gtk::gdk::Texture, String> {
    let widget = widget.as_ref();
    let (w, h) = (widget.width() as f64, widget.height() as f64);
    let snapshot = gtk::Snapshot::new();
    // Page content has no background of its own; use the window's.
    if !widget.is::<gtk::Window>() {
        let v = if adw::StyleManager::default().is_dark() { 0.14 } else { 0.98 };
        snapshot.append_color(&gtk::gdk::RGBA::new(v, v, v, 1.0), &gtk::graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
    }
    gtk::WidgetPaintable::new(Some(widget)).snapshot(&snapshot, w, h);
    let node = snapshot.to_node().ok_or("nothing to render")?;
    let renderer = gsk::CairoRenderer::new();
    renderer.realize_for_display(&WidgetExt::display(widget)).map_err(|e| e.to_string())?;
    let texture = renderer.render_texture(&node, Some(&node.bounds()));
    renderer.unrealize();
    Ok(texture)
}
