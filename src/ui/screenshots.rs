//! Renders each page to a PNG for store listings. Enabled by running the app
//! with BHC_SCREENSHOTS=<folder> (and optionally BHC_SCREENSHOTS_STYLE=light or
//! dark); it saves one image per page and quits.

use std::path::PathBuf;
use std::time::Duration;

use adw::prelude::*;
use gtk::{glib, gsk};

const PAGES: &[&str] = &["overview", "health", "charging", "usage"];

pub fn capture(window: &adw::ApplicationWindow, stack: &adw::ViewStack, dir: PathBuf, scroll_usage: impl Fn() + 'static) {
    let _ = std::fs::create_dir_all(&dir);
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
        capture_page(window, stack, dir, scroll_usage, i + 1);
    });
}

fn render(window: &adw::ApplicationWindow) -> Result<gtk::gdk::Texture, String> {
    let (w, h) = (window.width() as f64, window.height() as f64);
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(window)).snapshot(&snapshot, w, h);
    let node = snapshot.to_node().ok_or("nothing to render")?;
    let renderer = gsk::CairoRenderer::new();
    renderer.realize_for_display(&WidgetExt::display(window)).map_err(|e| e.to_string())?;
    let texture = renderer.render_texture(&node, Some(&node.bounds()));
    renderer.unrealize();
    Ok(texture)
}
