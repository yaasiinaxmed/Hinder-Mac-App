//! Hinder's design system: two looks (Vivid, Mono) × light/dark, plus fonts.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
pub enum Look {
    #[default]
    Vivid,
    Mono,
}

/// Every color the UI paints comes from here.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub look: Look,
    pub bg: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub thumb_bg: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub green: Color32,
    pub red: Color32,
    pub shadow: Color32,
}

/// Hinder's brand colours, taken from the copper folder icon (`assets/icon.svg`).
pub const COPPER: Color32 = Color32::from_rgb(0xE0, 0x5E, 0x2E); // between #E8692F glow and #CE4A26 folder
pub const COPPER_DEEP: Color32 = Color32::from_rgb(0xC2, 0x52, 0x2F); // folder back
pub const COPPER_LIGHT: Color32 = Color32::from_rgb(0xF0, 0xA2, 0x74); // folder front highlight

/// Six warm-leaning hues for shelves, sources and clean-up cards in the Vivid look.
/// Copper leads; the rest are chosen to sit comfortably next to it.
const HUES: [Color32; 6] = [
    Color32::from_rgb(0xE0, 0x5E, 0x2E), // copper   — Images
    Color32::from_rgb(0x3F, 0x8E, 0xC9), // slate blue — Videos
    Color32::from_rgb(0xD9, 0x9A, 0x2B), // brass    — Music
    Color32::from_rgb(0x2E, 0x9C, 0x84), // patina   — Documents
    Color32::from_rgb(0xB8, 0x6B, 0x3F), // bronze   — Archives
    Color32::from_rgb(0xC8, 0x4B, 0x6E), // garnet   — Apps
];

impl Palette {
    pub fn new(look: Look, dark: bool) -> Self {
        let (bg, sidebar, surface, border, border_strong, text, muted, faint, hover, selected, thumb_bg) = if dark {
            // Charcoal-brown, like the icon's background (#3B2219 → #140B08).
            (
                rgb(0x17, 0x10, 0x0D), // bg
                rgb(0x1F, 0x16, 0x12), // sidebar
                rgb(0x24, 0x1A, 0x15), // surface
                rgb(0x38, 0x2A, 0x23), // border
                rgb(0x52, 0x3F, 0x35), // border_strong
                rgb(0xF7, 0xEE, 0xE8), // text
                rgb(0xB9, 0xA6, 0x9B), // muted
                rgb(0x82, 0x6F, 0x64), // faint
                rgb(0x2E, 0x22, 0x1C), // hover
                rgb(0x3B, 0x2B, 0x23), // selected
                rgb(0x2B, 0x20, 0x1A), // thumb_bg
            )
        } else {
            // Warm paper whites, a nod to the sheet inside the folder (#F6E3D3).
            (
                rgb(0xFF, 0xFF, 0xFF), // bg
                rgb(0xFA, 0xF5, 0xF1), // sidebar
                rgb(0xFF, 0xFF, 0xFF), // surface
                rgb(0xEE, 0xE4, 0xDD), // border
                rgb(0xDB, 0xCB, 0xC0), // border_strong
                rgb(0x24, 0x17, 0x12), // text
                rgb(0x78, 0x65, 0x5B), // muted
                rgb(0xAA, 0x99, 0x8E), // faint
                rgb(0xF6, 0xEE, 0xE8), // hover
                rgb(0xF1, 0xE4, 0xDA), // selected
                rgb(0xF8, 0xF2, 0xEE), // thumb_bg
            )
        };
        let accent = match look {
            Look::Vivid => COPPER,
            Look::Mono => text,
        };
        Self {
            dark,
            look,
            bg,
            sidebar,
            surface,
            border,
            border_strong,
            text,
            muted,
            faint,
            hover,
            selected,
            thumb_bg,
            accent,
            on_accent: if look == Look::Mono { bg } else { Color32::WHITE },
            green: rgb(0x2E, 0x9C, 0x6A),
            red: rgb(0xD6, 0x3E, 0x3A),
            shadow: if dark { Color32::from_black_alpha(110) } else { Color32::from_rgba_unmultiplied(0x5A, 0x2A, 0x14, 22) },
        }
    }

    /// Strong colour for slot `i` (icons, dots).
    pub fn hue(&self, i: usize) -> Color32 {
        match self.look {
            Look::Vivid => HUES[i % HUES.len()],
            Look::Mono => self.muted,
        }
    }

    /// Soft background tint for slot `i`.
    pub fn tint(&self, i: usize) -> Color32 {
        match self.look {
            Look::Vivid => mix(self.surface, HUES[i % HUES.len()], if self.dark { 0.16 } else { 0.09 }),
            Look::Mono => self.thumb_bg,
        }
    }

    /// Border for a tinted card.
    pub fn tint_border(&self, i: usize) -> Color32 {
        match self.look {
            Look::Vivid => mix(self.surface, HUES[i % HUES.len()], if self.dark { 0.30 } else { 0.22 }),
            Look::Mono => self.border,
        }
    }

    pub fn kind_slot(kind: crate::model::Kind) -> usize {
        use crate::model::Kind::*;
        match kind {
            Image => 0,
            Video => 1,
            Music => 2,
            Document => 3,
            Archive => 4,
            App => 5,
            Other => 0,
        }
    }

    pub fn hairline(&self) -> Stroke {
        Stroke::new(1.0, self.border)
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

// ---------- typography ----------

pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn f(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn fm(size: f32) -> FontId {
    FontId::new(size, medium())
}
pub fn fb(size: f32) -> FontId {
    FontId::new(size, bold())
}

fn load(path: &str, index: u32) -> Option<FontData> {
    let bytes = std::fs::read(path).ok()?;
    let mut fd = FontData::from_owned(bytes);
    fd.index = index;
    Some(fd)
}

/// Uses the Mac's own San Francisco / Helvetica Neue faces when present, and
/// falls back to egui's bundled fonts elsewhere. Phosphor icons are appended to
/// every family so icons can be mixed into any text.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    let mut reg = vec![];
    if let Some(fd) = load("/System/Library/Fonts/SFNS.ttf", 0) {
        fonts.font_data.insert("sf".into(), Arc::new(fd));
        reg.push("sf".to_string());
    }
    let mut med = vec![];
    if let Some(fd) = load("/System/Library/Fonts/HelveticaNeue.ttc", 10) {
        fonts.font_data.insert("hn-medium".into(), Arc::new(fd));
        med.push("hn-medium".to_string());
    }
    let mut bld = vec![];
    if let Some(fd) = load("/System/Library/Fonts/HelveticaNeue.ttc", 1) {
        fonts.font_data.insert("hn-bold".into(), Arc::new(fd));
        bld.push("hn-bold".to_string());
    }

    let base = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let with_base = |mut v: Vec<String>| {
        for b in &base {
            if !v.contains(b) {
                v.push(b.clone());
            }
        }
        v
    };
    let regular = with_base(reg);
    fonts.families.insert(FontFamily::Proportional, regular.clone());
    fonts.families.insert(medium(), with_base(if med.is_empty() { regular.clone() } else { med }));
    fonts.families.insert(bold(), with_base(if bld.is_empty() { regular.clone() } else { bld }));
    ctx.set_fonts(fonts);
}

pub fn apply_visuals(ctx: &egui::Context, p: &Palette) {
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.thumb_bg;
    v.override_text_color = Some(p.text);
    v.window_stroke = p.hairline();
    v.window_corner_radius = egui::CornerRadius::same(14);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 60);
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.widgets.noninteractive.bg_stroke = p.hairline();
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = egui::CornerRadius::same(8);
    }
    v.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
    v.widgets.inactive.bg_fill = p.thumb_bg;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.weak_bg_fill = p.selected;
    v.widgets.active.bg_stroke = Stroke::NONE;
    v.widgets.open.weak_bg_fill = p.selected;
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 24, spread: 0, color: p.shadow };
    v.window_shadow = egui::epaint::Shadow { offset: [0, 10], blur: 40, spread: 0, color: p.shadow };
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.interaction.selectable_labels = false;
        s.text_styles.insert(egui::TextStyle::Body, f(13.5));
        s.text_styles.insert(egui::TextStyle::Button, f(13.5));
        s.text_styles.insert(egui::TextStyle::Small, f(11.5));
        s.text_styles.insert(egui::TextStyle::Heading, fb(26.0));
    });
}
