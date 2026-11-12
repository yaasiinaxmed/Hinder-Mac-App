//! Card components shared by several screens.

use eframe::egui::{self, Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use egui_phosphor::regular as ph;

use super::widgets::*;
use crate::cleanup::Card;
use crate::model::{FileEntry, Kind, fmt_count, fmt_date, fmt_size};
use crate::theme::{Look, Palette, f, fb, fm, mix};
use crate::thumbs::Thumbs;

/// Lay out `n` equal cards in rows. Returns (columns, card width).
pub fn grid_dims(avail: f32, min_w: f32, gap: f32, max_cols: usize) -> (usize, f32) {
    let cols = (((avail + gap) / (min_w + gap)).floor() as usize).clamp(1, max_cols);
    let w = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
    (cols, w)
}

fn card_frame(ui: &Ui, p: &Palette, rect: Rect, resp: &Response, selected: bool, fill: Color32, border: Color32, radius: u8) {
    let painter = ui.painter();
    if resp.hovered() {
        painter.add(egui::epaint::RectShape::filled(rect.translate(vec2(0.0, 2.0)), CornerRadius::same(radius), p.shadow).with_blur_width(10.0));
    }
    let stroke = if selected {
        Stroke::new(2.0, p.accent)
    } else if resp.hovered() {
        Stroke::new(1.0, p.border_strong)
    } else {
        Stroke::new(1.0, border)
    };
    painter.rect(rect, CornerRadius::same(radius), fill, stroke, StrokeKind::Inside);
}

pub fn badge_text(f: &FileEntry) -> String {
    match f.dims {
        Some((w, h)) => format!("{} · {}×{}", f.type_label(), w, h),
        None => format!("{} · {}", f.type_label(), fmt_size(f.size)),
    }
}

/// A file as a big thumbnail card.
pub fn file_card(ui: &mut Ui, p: &Palette, thumbs: &mut Thumbs, file: &FileEntry, size: Vec2, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    card_frame(ui, p, rect, &resp, selected, p.surface, p.border, 14);
    let painter = ui.painter_at(rect.expand(4.0));
    let text_h = 58.0;
    let thumb = Rect::from_min_max(rect.min + vec2(1.0, 1.0), pos2(rect.right() - 1.0, rect.bottom() - text_h));
    painter.rect_filled(thumb, CornerRadius { nw: 13, ne: 13, sw: 0, se: 0 }, p.thumb_bg);

    let slot = Palette::kind_slot(file.kind);
    match thumbs.get(&file.path) {
        Some(t) => {
            let inner = Rect::from_min_max(thumb.min + vec2(18.0, 38.0), thumb.max - vec2(18.0, 14.0));
            let r = image_contain(&painter, t, inner, CornerRadius::same(5));
            painter.rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, p.border), StrokeKind::Outside);
        }
        None => {
            painter.text(thumb.center() + vec2(0.0, 10.0), Align2::CENTER_CENTER, file.kind.icon(), f(thumb.height() * 0.3), mix(p.thumb_bg, p.hue(slot), 0.6));
        }
    }
    let dot = if p.look == Look::Vivid { p.hue(slot) } else { p.green };
    badge(&painter, p, thumb.min + vec2(10.0, 10.0), &badge_text(file), Some(dot));

    let name_font = fm(13.5);
    let name = middle_truncate(ui, &file.name, &name_font, rect.width() - 28.0);
    painter.text(pos2(rect.left() + 14.0, thumb.bottom() + 12.0), Align2::LEFT_TOP, name, name_font, p.text);
    painter.text(pos2(rect.left() + 14.0, thumb.bottom() + 32.0), Align2::LEFT_TOP, fmt_date(file.modified), f(12.0), p.faint);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A file as a list row.
pub fn file_row(ui: &mut Ui, p: &Palette, thumbs: &mut Thumbs, file: &FileEntry, height: f32, selected: bool, odd: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let painter = ui.painter();
    let fill = if selected {
        mix(p.surface, p.accent, if p.dark { 0.25 } else { 0.12 })
    } else if resp.hovered() {
        p.hover
    } else if odd {
        mix(p.bg, p.thumb_bg, 0.55)
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, CornerRadius::same(8), fill);
    let t = Rect::from_min_size(pos2(rect.left() + 10.0, rect.center().y - 14.0), vec2(28.0, 28.0));
    match thumbs.get(&file.path) {
        Some(tex) => image_cover(painter, tex, t, CornerRadius::same(5)),
        None => {
            painter.rect_filled(t, CornerRadius::same(5), p.thumb_bg);
            painter.text(t.center(), Align2::CENTER_CENTER, file.kind.icon(), f(15.0), p.hue(Palette::kind_slot(file.kind)));
        }
    }
    let w = rect.width();
    let cols = [w * 0.46, w * 0.14, w * 0.12];
    let x0 = t.right() + 12.0;
    let name = middle_truncate(ui, &file.name, &f(13.5), cols[0] - 50.0);
    painter.text(pos2(x0, rect.center().y), Align2::LEFT_CENTER, name, f(13.5), p.text);
    let x1 = rect.left() + cols[0];
    painter.text(pos2(x1, rect.center().y), Align2::LEFT_CENTER, file.type_label(), f(12.5), p.muted);
    let x2 = x1 + cols[1];
    painter.text(pos2(x2, rect.center().y), Align2::LEFT_CENTER, fmt_size(file.size), f(12.5), p.muted);
    let x3 = x2 + cols[2];
    painter.text(pos2(x3, rect.center().y), Align2::LEFT_CENTER, fmt_date(file.modified), f(12.5), p.muted);
    resp
}

pub fn list_header(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let w = rect.width();
    let y = rect.center().y;
    let font = fm(11.5);
    let x0 = rect.left() + 50.0;
    let x1 = rect.left() + w * 0.46;
    let x2 = x1 + w * 0.14;
    let x3 = x2 + w * 0.12;
    for (x, t) in [(x0, "Name"), (x1, "Kind"), (x2, "Size"), (x3, "Date Modified")] {
        ui.painter().text(pos2(x, y), Align2::LEFT_CENTER, t, font.clone(), p.faint);
    }
    ui.painter().line_segment([pos2(rect.left(), rect.bottom()), pos2(rect.right(), rect.bottom())], p.hairline());
}

/// Library shelf: a tinted card with a fanned stack of recent prints.
pub fn shelf_card(ui: &mut Ui, p: &Palette, thumbs: &mut Thumbs, kind: Kind, count: usize, bytes: u64, samples: &[std::path::PathBuf], size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let slot = Palette::kind_slot(kind);
    let fill = if p.look == Look::Vivid { p.tint(slot) } else { p.surface };
    card_frame(ui, p, rect, &resp, false, fill, p.tint_border(slot), 18);
    let painter = ui.painter_at(rect);
    let stage = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 64.0));
    let c = stage.center() + vec2(0.0, 8.0);
    let ph_h = (stage.height() * 0.62).min(96.0);
    let print = vec2(ph_h * 1.25, ph_h);
    let lift = if resp.hovered() { 1.25 } else { 1.0 };
    let tint = p.hue(slot);
    let fallback = kind.icon();
    let tex: Vec<Option<egui::TextureHandle>> = (0..3).map(|i| samples.get(i).and_then(|s| thumbs.get(s).cloned())).collect();
    let spread = print.x * 0.32 * lift;
    polaroid(&painter, p, tex.get(2).and_then(|t| t.as_ref()), c + vec2(spread, 4.0), print * 0.9, 0.16 * lift, fallback, tint);
    polaroid(&painter, p, tex.get(1).and_then(|t| t.as_ref()), c + vec2(-spread, 4.0), print * 0.9, -0.16 * lift, fallback, tint);
    polaroid(&painter, p, tex.first().and_then(|t| t.as_ref()), c - vec2(0.0, 2.0 * lift), print, 0.0, fallback, tint);

    let base = pos2(rect.left() + 18.0, rect.bottom() - 46.0);
    let (ir, _) = (Rect::from_min_size(base, vec2(26.0, 26.0)), ());
    painter.rect_filled(ir, CornerRadius::same(8), if p.look == Look::Vivid { mix(p.surface, tint, 0.18) } else { p.thumb_bg });
    painter.text(ir.center(), Align2::CENTER_CENTER, kind.icon(), f(15.0), if p.look == Look::Vivid { tint } else { p.text });
    painter.text(pos2(ir.right() + 10.0, ir.top() - 1.0), Align2::LEFT_TOP, kind.label(), fm(15.0), p.text);
    painter.text(pos2(ir.right() + 10.0, ir.top() + 18.0), Align2::LEFT_TOP, format!("{} items · {}", fmt_count(count), fmt_size(bytes)), f(12.0), p.muted);
    painter.text(pos2(rect.right() - 18.0, ir.center().y), Align2::RIGHT_CENTER, ph::ARROW_RIGHT, f(15.0), if resp.hovered() { p.text } else { p.faint });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Clean Up suggestion card.
pub fn cleanup_card(ui: &mut Ui, p: &Palette, thumbs: &mut Thumbs, card: &Card, share: f32, size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let slot = card.kind.tint_index();
    let empty = card.paths.is_empty();
    let fill = if p.look == Look::Vivid && !empty { p.tint(slot) } else { p.surface };
    card_frame(ui, p, rect, &resp, false, fill, if empty { p.border } else { p.tint_border(slot) }, 16);
    let painter = ui.painter_at(rect);
    let hue = p.hue(slot);
    let pad = 16.0;
    let ir = Rect::from_min_size(rect.min + vec2(pad, pad), vec2(34.0, 34.0));
    painter.rect_filled(ir, CornerRadius::same(10), if p.look == Look::Vivid { mix(p.surface, hue, 0.2) } else { p.thumb_bg });
    painter.text(ir.center(), Align2::CENTER_CENTER, card.kind.icon(), f(17.0), if p.look == Look::Vivid { hue } else { p.text });

    // little stack of samples
    for (i, path) in card.paths.iter().take(3).enumerate().collect::<Vec<_>>().into_iter().rev() {
        let cx = rect.right() - pad - 12.0 - i as f32 * 17.0;
        circle_thumb(&painter, p, thumbs.get(path), pos2(cx, ir.center().y), 12.0, ph::FILE, p.muted);
    }

    let text_col = if empty { p.faint } else { p.text };
    painter.text(pos2(rect.left() + pad, ir.bottom() + 14.0), Align2::LEFT_TOP, card.kind.title(), f(12.5), p.muted);
    let value = if empty { "All clear".to_string() } else { fmt_size(card.bytes) };
    painter.text(pos2(rect.left() + pad, ir.bottom() + 32.0), Align2::LEFT_TOP, value, fb(25.0), text_col);
    if !empty {
        painter.text(pos2(rect.right() - pad, ir.bottom() + 46.0), Align2::RIGHT_CENTER, format!("{}  {}", card.count_label(), ph::CARET_RIGHT), f(12.0), p.muted);
    }
    // share-of-total bar
    let bar = Rect::from_min_size(pos2(rect.left() + pad, rect.bottom() - pad - 4.0), vec2(rect.width() - pad * 2.0, 4.0));
    painter.rect_filled(bar, CornerRadius::same(2), if p.look == Look::Vivid { mix(p.surface, hue, 0.18) } else { p.thumb_bg });
    if share > 0.0 {
        let mut fillr = bar;
        fillr.set_width((bar.width() * share.clamp(0.0, 1.0)).max(4.0));
        painter.rect_filled(fillr, CornerRadius::same(2), if p.look == Look::Vivid { hue } else { p.text });
    }
    resp.on_hover_text(card.kind.explain()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// "Downloaded From" source card.
pub fn source_card(ui: &mut Ui, p: &Palette, thumbs: &mut Thumbs, s: &crate::app::SourceSummary, size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    card_frame(ui, p, rect, &resp, false, p.surface, p.border, 14);
    let painter = ui.painter_at(rect);
    let slot = s.key.bytes().fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    let hue = p.hue(slot);
    let c = pos2(rect.left() + 34.0, rect.center().y);
    painter.circle_filled(c, 18.0, if p.look == Look::Vivid { mix(p.surface, hue, 0.16) } else { p.thumb_bg });
    painter.text(c, Align2::CENTER_CENTER, crate::sources::icon_for(&s.key), f(18.0), if p.look == Look::Vivid { hue } else { p.text });

    let thumbs_w = if s.samples.is_empty() { 0.0 } else { 30.0 + 20.0 * (s.samples.len().min(2) as f32 - 1.0) + 8.0 };
    let tx = c.x + 30.0;
    let max_w = rect.right() - tx - thumbs_w - 12.0;
    let name = end_truncate(ui, &s.label, &fm(14.0), max_w);
    painter.text(pos2(tx, rect.center().y - 9.0), Align2::LEFT_CENTER, name, fm(14.0), p.text);
    let sub = end_truncate(ui, &format!("{} · {}", crate::model::plural(s.count, "file", "files"), fmt_size(s.bytes)), &f(12.0), max_w);
    painter.text(pos2(tx, rect.center().y + 10.0), Align2::LEFT_CENTER, sub, f(12.0), p.muted);
    for (i, path) in s.samples.iter().take(2).enumerate().collect::<Vec<_>>().into_iter().rev() {
        let cx = rect.right() - 26.0 - i as f32 * 20.0;
        circle_thumb(&painter, p, thumbs.get(path), pos2(cx, rect.center().y), 15.0, ph::FILE, p.muted);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
