//! Small reusable painted widgets.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextureHandle, Ui, Vec2,
    epaint::RectShape, pos2, vec2,
};

use eframe::emath::Rot2;

use crate::theme::{Palette, f, fm};

pub fn text_width(ui: &Ui, text: &str, font: &FontId) -> f32 {
    ui.fonts(|fo| fo.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE).size().x)
}

/// "screencapture-se…0-01-22_14_38.pdf": keeps the start and the extension visible.
pub fn middle_truncate(ui: &Ui, text: &str, font: &FontId, max_w: f32) -> String {
    if text_width(ui, text, font) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (1usize, chars.len());
    let make = |keep: usize| -> String {
        let tail = (keep * 2 / 5).max(1);
        let head = keep.saturating_sub(tail);
        let mut s: String = chars[..head].iter().collect();
        s.push('…');
        s.extend(&chars[chars.len() - tail..]);
        s
    };
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if text_width(ui, &make(mid), font) <= max_w { lo = mid } else { hi = mid - 1 }
    }
    make(lo)
}

pub fn end_truncate(ui: &Ui, text: &str, font: &FontId, max_w: f32) -> String {
    if text_width(ui, text, font) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let s: String = chars[..mid].iter().collect::<String>() + "…";
        if text_width(ui, &s, font) <= max_w { lo = mid } else { hi = mid - 1 }
    }
    chars[..lo].iter().collect::<String>() + "…"
}

/// Rect of size `content` fitted inside `outer`, keeping aspect ratio, centred.
pub fn fit(outer: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return outer;
    }
    let s = (outer.width() / content.x).min(outer.height() / content.y);
    Rect::from_center_size(outer.center(), content * s)
}

/// UV rect that crops a texture of `tex` size to fill an area with aspect `want`.
pub fn cover_uv(tex: Vec2, want: Vec2) -> Rect {
    let (ta, wa) = (tex.x / tex.y, want.x / want.y);
    if ta > wa {
        let w = wa / ta;
        Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = ta / wa;
        Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
    }
}

pub fn tex_size(t: &TextureHandle) -> Vec2 {
    let [w, h] = t.size();
    vec2(w as f32, h as f32)
}

/// Draw a texture into `rect` with rounded corners, cropping to fill.
pub fn image_cover(painter: &egui::Painter, tex: &TextureHandle, rect: Rect, radius: impl Into<CornerRadius>) {
    let uv = cover_uv(tex_size(tex), rect.size());
    painter.add(RectShape::filled(rect, radius, Color32::WHITE).with_texture(tex.id(), uv));
}

/// Draw a texture inside `rect` keeping its whole frame visible.
pub fn image_contain(painter: &egui::Painter, tex: &TextureHandle, rect: Rect, radius: impl Into<CornerRadius>) -> Rect {
    let r = fit(rect, tex_size(tex));
    painter.add(RectShape::filled(r, radius, Color32::WHITE).with_texture(tex.id(), Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0))));
    r
}

/// A circular avatar-style thumbnail with a ring, or a fallback glyph.
pub fn circle_thumb(painter: &egui::Painter, p: &Palette, tex: Option<&TextureHandle>, center: Pos2, r: f32, fallback: &str, tint: Color32) {
    let rect = Rect::from_center_size(center, Vec2::splat(r * 2.0));
    let cr = CornerRadius::same(r.min(127.0) as u8);
    painter.rect_filled(rect.expand(2.0), CornerRadius::same((r + 2.0).min(127.0) as u8), p.surface);
    match tex {
        Some(t) => image_cover(painter, t, rect, cr),
        None => {
            painter.rect_filled(rect, cr, p.thumb_bg);
            painter.text(center, Align2::CENTER_CENTER, fallback, f(r * 0.95), tint);
        }
    }
}

/// A rotated "print" (photo with white border) used on the library shelves.
pub fn polaroid(painter: &egui::Painter, p: &Palette, tex: Option<&TextureHandle>, center: Pos2, size: Vec2, angle: f32, fallback: &str, tint: Color32) {
    let rot = Rot2::from_angle(angle);
    let frame = Rect::from_center_size(center, size);
    let border = 4.0;
    // shadow
    let mut sh = Mesh::default();
    sh.add_colored_rect(frame.translate(vec2(0.0, 3.0)).expand(1.0), p.shadow);
    sh.rotate(rot, center);
    painter.add(Shape::mesh(sh));
    // paper
    let mut paper = Mesh::default();
    let paper_col = if p.dark { Color32::from_gray(58) } else { Color32::WHITE };
    paper.add_colored_rect(frame, paper_col);
    paper.rotate(rot, center);
    painter.add(Shape::mesh(paper));
    let inner = frame.shrink(border);
    match tex {
        Some(t) => {
            let mut m = Mesh::with_texture(t.id());
            m.add_rect_with_uv(inner, cover_uv(tex_size(t), inner.size()), Color32::WHITE);
            m.rotate(rot, center);
            painter.add(Shape::mesh(m));
        }
        None => {
            let mut m = Mesh::default();
            m.add_colored_rect(inner, p.thumb_bg);
            m.rotate(rot, center);
            painter.add(Shape::mesh(m));
            painter.text(center, Align2::CENTER_CENTER, fallback, f(size.y * 0.32), tint);
        }
    }
}

/// Little rounded label: "● PDF · 776 KB".
pub fn badge(painter: &egui::Painter, p: &Palette, pos: Pos2, text: &str, dot: Option<Color32>) -> Rect {
    let font = fm(11.0);
    let galley = painter.layout_no_wrap(text.to_owned(), font, p.muted);
    let dot_w = if dot.is_some() { 12.0 } else { 0.0 };
    let rect = Rect::from_min_size(pos, vec2(galley.size().x + 16.0 + dot_w, 20.0));
    let fill = if p.dark { Color32::from_black_alpha(140) } else { Color32::from_white_alpha(235) };
    painter.rect(rect, CornerRadius::same(10), fill, Stroke::new(1.0, p.border), StrokeKind::Inside);
    if let Some(c) = dot {
        painter.circle_filled(pos2(rect.left() + 11.0, rect.center().y), 3.0, c);
    }
    painter.galley(pos2(rect.left() + 8.0 + dot_w, rect.center().y - galley.size().y / 2.0), galley, p.muted);
    rect
}

/// A filled pill with strong text (counts, "Free up to …").
pub fn pill(ui: &mut Ui, text: &str, fill: Color32, color: Color32) -> Response {
    let font = fm(12.0);
    let w = text_width(ui, text, &font) + 18.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(11), fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, text, font, color);
    resp
}

/// Keyboard key cap.
pub fn kbd(ui: &mut Ui, p: &Palette, key: &str) {
    let font = fm(11.0);
    let w = (text_width(ui, key, &font) + 10.0).max(20.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(5), p.thumb_bg, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, key, font, p.muted);
}

/// Outlined button with an optional leading icon.
pub fn outline_button(ui: &mut Ui, p: &Palette, icon: Option<&str>, text: &str) -> Response {
    let font = f(13.0);
    let label = match icon {
        Some(i) => format!("{i}  {text}"),
        None => text.to_owned(),
    };
    let w = text_width(ui, &label, &font) + 26.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let fill = if resp.hovered() { p.hover } else { p.surface };
    ui.painter().rect(rect, CornerRadius::same(9), fill, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, p.text);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Solid button (primary or danger).
pub fn solid_button(ui: &mut Ui, fill: Color32, color: Color32, icon: Option<&str>, text: &str) -> Response {
    let font = fm(13.0);
    let label = match icon {
        Some(i) => format!("{i}  {text}"),
        None => text.to_owned(),
    };
    let w = text_width(ui, &label, &font) + 28.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let fill = if resp.hovered() { crate::theme::mix(fill, Color32::BLACK, 0.08) } else { fill };
    ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Square icon button (toolbar).
pub fn icon_button(ui: &mut Ui, p: &Palette, icon: &str, active: bool, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    let fill = if active { p.selected } else if resp.hovered() { p.hover } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, f(17.0), if active { p.text } else { p.muted });
    resp.on_hover_text(tip)
}

/// Filter pill "Kind: All ⌄" that opens a popup with options.
pub fn dropdown<T: Clone + PartialEq>(ui: &mut Ui, p: &Palette, id: &str, label: &str, current: &str, options: &[(String, T)], value: &mut T) -> bool {
    let font_l = f(13.0);
    let font_v = fm(13.0);
    let caret = egui_phosphor::regular::CARET_DOWN;
    let w = text_width(ui, &format!("{label}: "), &font_l) + text_width(ui, current, &font_v) + 40.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let popup_id = ui.make_persistent_id(id);
    let open = ui.memory(|m| m.is_popup_open(popup_id));
    let fill = if open { p.selected } else if resp.hovered() { p.hover } else { p.surface };
    ui.painter().rect(rect, CornerRadius::same(9), fill, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut x = rect.left() + 12.0;
    let g1 = ui.painter().layout_no_wrap(format!("{label}: "), font_l, p.muted);
    let g1w = g1.size().x;
    ui.painter().galley(pos2(x, rect.center().y - g1.size().y / 2.0), g1, p.muted);
    x += g1w;
    let g2 = ui.painter().layout_no_wrap(current.to_owned(), font_v, p.text);
    ui.painter().galley(pos2(x, rect.center().y - g2.size().y / 2.0), g2, p.text);
    ui.painter().text(pos2(rect.right() - 14.0, rect.center().y), Align2::CENTER_CENTER, caret, f(12.0), p.muted);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    let mut changed = false;
    egui::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(170.0);
        for (text, v) in options {
            let sel = v == value;
            let row = ui.selectable_label(sel, format!("{}  {}", if sel { egui_phosphor::regular::CHECK } else { "   " }, text));
            if row.clicked() && !sel {
                *value = v.clone();
                changed = true;
            }
        }
    });
    changed
}

/// Removable chip used for "Understood as".
pub fn chip(ui: &mut Ui, p: &Palette, text: &str, slot: usize) -> bool {
    let font = fm(12.5);
    let x_icon = egui_phosphor::regular::X;
    let w = text_width(ui, text, &font) + 40.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    let fill = p.tint(slot);
    let stroke = Stroke::new(1.0, if resp.hovered() { p.hue(slot) } else { p.tint_border(slot) });
    ui.painter().rect(rect, CornerRadius::same(13), fill, stroke, StrokeKind::Inside);
    ui.painter().text(pos2(rect.left() + 12.0, rect.center().y), Align2::LEFT_CENTER, text, font, p.text);
    ui.painter().text(pos2(rect.right() - 14.0, rect.center().y), Align2::CENTER_CENTER, x_icon, f(11.0), if resp.hovered() { p.text } else { p.faint });
    resp.on_hover_text("Remove this filter").on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// Section title with an optional count bubble.
pub fn section_title(ui: &mut Ui, p: &Palette, title: &str, count: Option<String>) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(crate::theme::fm(20.0)).color(p.text));
        if let Some(c) = count {
            ui.add_space(4.0);
            pill(ui, &c, p.thumb_bg, p.muted);
        }
    });
}

/// Hinder logo: the copper folder icon rendered from `assets/icon.svg`.
pub fn logo(painter: &egui::Painter, tex: Option<&TextureHandle>, rect: Rect) {
    match tex {
        Some(t) => {
            painter.image(t.id(), rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            painter.rect_filled(rect, CornerRadius::same((rect.width() * 0.22) as u8), crate::theme::COPPER);
        }
    }
}
