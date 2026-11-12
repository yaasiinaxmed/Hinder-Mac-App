//! Hinder: a friendlier way to see, search and tidy the files on your Mac.
//!
//! Open source, private by design: everything runs locally.

mod actions;
mod app;
mod brand;
mod cleanup;
mod dupes;
mod menu;
mod model;
mod query;
mod scanner;
mod sources;
mod theme;
mod thumbs;
mod ui;

fn main() -> eframe::Result<()> {
    // `hinder --export-icon <out.png> <px>` renders the app icon (used for packaging).
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--export-icon") {
        let out = args.get(2).map(String::as_str).unwrap_or("icon.png");
        let px = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1024);
        if let Err(e) = brand::export_png(out, px) {
            eprintln!("export failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Hinder")
        .with_app_id("app.hinder")
        .with_inner_size([1320.0, 860.0])
        .with_min_inner_size([820.0, 560.0])
        .with_fullsize_content_view(true)
        .with_titlebar_shown(false)
        .with_title_shown(false)
        .with_icon(std::sync::Arc::new(brand::window_icon()));

    let options = eframe::NativeOptions { viewport, persist_window: true, ..Default::default() };
    eframe::run_native("Hinder", options, Box::new(|cc| Ok(Box::new(app::HinderApp::new(cc)))))
}
