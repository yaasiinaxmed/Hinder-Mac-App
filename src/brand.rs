//! The Hinder icon and logo, rendered from SVG at whatever size is needed.
//!
//! * `assets/icon.svg` is the app (Dock) icon. It follows the macOS icon grid:
//!   an 824 px squircle centred on a 1024 px canvas with a soft drop shadow, so
//!   it sits at the same size as every other app in the Dock.
//! * `assets/logo.svg` is the same artwork edge-to-edge, for drawing inside the UI.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use resvg::{tiny_skia, usvg};

pub const ICON_SVG: &[u8] = include_bytes!("../assets/icon.svg");
pub const LOGO_SVG: &[u8] = include_bytes!("../assets/logo.svg");

/// Rasterise an SVG to a square pixmap of `px` × `px`.
fn render(svg: &[u8], px: u32) -> Option<tiny_skia::Pixmap> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    let s = px as f32 / tree.size().width();
    resvg::render(&tree, tiny_skia::Transform::from_scale(s, s), &mut pixmap.as_mut());
    Some(pixmap)
}

/// Window / Dock icon.
pub fn window_icon() -> egui::IconData {
    let px = 512;
    let rgba = render(ICON_SVG, px)
        .map(|p| {
            p.pixels()
                .iter()
                .flat_map(|c| {
                    let c = c.demultiply();
                    [c.red(), c.green(), c.blue(), c.alpha()]
                })
                .collect()
        })
        .unwrap_or_default();
    egui::IconData { rgba, width: px, height: px }
}

/// Texture for drawing the logo inside the UI (rendered at 2× for Retina).
pub fn logo_texture(ctx: &egui::Context, points: u32) -> Option<TextureHandle> {
    let px = points * 2;
    let p = render(LOGO_SVG, px)?;
    let img = ColorImage::from_rgba_premultiplied([px as usize, px as usize], p.data());
    Some(ctx.load_texture("hinder-logo", img, TextureOptions::LINEAR))
}

/// `hinder --export-icon out.png 1024`: used by `scripts/bundle.sh` to build the `.icns`.
pub fn export_png(path: &str, px: u32) -> Result<(), String> {
    let p = render(ICON_SVG, px).ok_or("could not render icon")?;
    p.save_png(path).map_err(|e| e.to_string())
}
