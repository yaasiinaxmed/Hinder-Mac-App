//! Duplicates: identical files grouped, one marked Keep, the rest Trash.

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Layout, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use egui_phosphor::regular as ph;

use super::Act;
use super::widgets::*;
use crate::app::HinderApp;
use crate::dupes;
use crate::model::{fmt_count, fmt_size, plural, tildify};
use crate::theme::{Look, f, fb, fm, mix};

const TILE: egui::Vec2 = egui::Vec2::new(176.0, 128.0);

pub fn show(app: &mut HinderApp, ui: &mut Ui, acts: &mut Vec<Act>) {
    let p = app.p;
    ui.add_space(4.0);
    ui.label(RichText::new("Duplicates").font(fb(28.0)).color(p.text));
    if !app.dups_ready {
        ui.label(RichText::new("Comparing file fingerprints (SHA-256), on this Mac…").color(p.muted));
        ui.add_space(30.0);
        ui.add(egui::Spinner::new().size(22.0).color(p.muted));
        return;
    }
    let total = dupes::total_reclaimable(&app.dups);
    let extra = dupes::extra_copies(&app.dups);
    ui.label(RichText::new(format!("{} with identical copies · iCloud-only files are skipped", plural(app.dups.len(), "file", "files"))).color(p.muted));
    ui.add_space(18.0);

    if app.dups.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(ph::CHECK_CIRCLE).font(f(44.0)).color(p.green));
            ui.add_space(8.0);
            ui.label(RichText::new("No duplicates. One copy of everything.").font(fm(17.0)).color(p.text));
        });
        return;
    }

    // ---- banner
    let banner_fill = if p.look == Look::Vivid { mix(p.surface, p.green, if p.dark { 0.18 } else { 0.10 }) } else { p.thumb_bg };
    egui::Frame::new().fill(banner_fill).corner_radius(CornerRadius::same(14)).inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(ph::SPARKLE).font(f(20.0)).color(if p.look == Look::Vivid { p.green } else { p.text }));
            ui.vertical(|ui| {
                ui.label(RichText::new(format!("Free up {}", fmt_size(total))).font(fm(16.0)).color(p.text));
                ui.label(RichText::new(format!("{} can go to the Trash. Click a copy to keep that one instead.", plural(extra, "extra copy", "extra copies"))).color(p.muted).font(f(12.5)));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if solid_button(ui, p.red, Color32::WHITE, Some(ph::TRASH), &format!("Trash {} Extra Copies", fmt_count(extra))).clicked() {
                    let all: Vec<_> = app.dups.iter().flat_map(|g| g.paths.iter().enumerate().filter(|(i, _)| *i != g.keep).map(|(_, p)| p.clone())).collect();
                    acts.push(Act::Trash(all));
                }
            });
        });
    });
    ui.add_space(16.0);

    // ---- groups (fixed height rows → virtualised)
    let group_h = 74.0 + TILE.y + 26.0;
    ui.spacing_mut().item_spacing.y = 14.0;
    let n = app.dups.len();
    let mut set_keep: Option<(usize, usize)> = None;
    ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, group_h, n, |ui, range| {
        for gi in range {
            let g = app.dups[gi].clone();
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), group_h), Sense::hover());
            ui.painter().rect(rect, CornerRadius::same(14), p.surface, Stroke::new(1.0, p.border), StrokeKind::Inside);
            let name = g.paths[g.keep].file_name().unwrap_or_default().to_string_lossy().to_string();
            ui.painter().text(rect.min + vec2(18.0, 16.0), Align2::LEFT_TOP, &name, fm(14.0), p.text);
            ui.painter().text(
                rect.min + vec2(18.0, 36.0),
                Align2::LEFT_TOP,
                format!("{} identical copies · {} can be freed", g.paths.len(), fmt_size(g.reclaimable())),
                f(12.5),
                p.muted,
            );
            // "Keep 1, Trash N" button
            let label = format!("{}  Keep 1, Trash {}", ph::TRASH, g.paths.len() - 1);
            let bw = text_width(ui, &label, &fm(12.5)) + 24.0;
            let br = Rect::from_min_size(pos2(rect.right() - 16.0 - bw, rect.top() + 16.0), vec2(bw, 30.0));
            let bresp = ui.interact(br, ui.id().with(("keep1", gi)), Sense::click());
            ui.painter().rect(br, CornerRadius::same(8), if bresp.hovered() { p.hover } else { p.thumb_bg }, Stroke::new(1.0, p.border), StrokeKind::Inside);
            ui.painter().text(br.center(), Align2::CENTER_CENTER, label, fm(12.5), p.text);
            if bresp.clicked() {
                acts.push(Act::Trash(g.paths.iter().enumerate().filter(|(i, _)| *i != g.keep).map(|(_, p)| p.clone()).collect()));
            }

            // tiles
            let mut x = rect.left() + 16.0;
            let y = rect.top() + 62.0;
            let max_tiles = (((rect.width() - 32.0) + 12.0) / (TILE.x + 12.0)).floor().max(1.0) as usize;
            for (ti, path) in g.paths.iter().enumerate().take(max_tiles) {
                let tr = Rect::from_min_size(pos2(x, y), vec2(TILE.x, TILE.y + 26.0));
                let keep = ti == g.keep;
                let resp = ui.interact(tr, ui.id().with(("tile", gi, ti)), Sense::click());
                let border = if keep { p.green } else if resp.hovered() { p.border_strong } else { p.border };
                ui.painter().rect(tr, CornerRadius::same(10), if keep { mix(p.surface, p.green, 0.06) } else { p.thumb_bg }, Stroke::new(if keep { 2.0 } else { 1.0 }, border), StrokeKind::Inside);
                let img = Rect::from_min_size(tr.min + vec2(8.0, 8.0), vec2(TILE.x - 16.0, TILE.y - 12.0));
                match app.thumbs.get(path) {
                    Some(t) => image_cover(ui.painter(), t, img, CornerRadius::same(6)),
                    None => {
                        ui.painter().rect_filled(img, CornerRadius::same(6), p.surface);
                        ui.painter().text(img.center(), Align2::CENTER_CENTER, ph::FILE, f(28.0), p.faint);
                    }
                }
                // Keep / Trash badge
                let (bt, bc) = if keep { ("Keep", p.green) } else { ("Trash", p.red) };
                let bw = text_width(ui, bt, &fm(11.0)) + 14.0;
                let b = Rect::from_min_size(img.min + vec2(6.0, 6.0), vec2(bw, 18.0));
                ui.painter().rect_filled(b, CornerRadius::same(9), bc);
                ui.painter().text(b.center(), Align2::CENTER_CENTER, bt, fm(11.0), Color32::WHITE);
                let loc = path.parent().map(tildify).unwrap_or_default();
                let loc = end_truncate(ui, &loc, &f(11.5), TILE.x - 16.0);
                ui.painter().text(pos2(tr.left() + 8.0, img.bottom() + 8.0), Align2::LEFT_TOP, loc, f(11.5), p.muted);
                let resp = resp.on_hover_text(path.display().to_string());
                if resp.clicked() {
                    set_keep = Some((gi, ti));
                }
                if resp.double_clicked() {
                    acts.push(Act::Open(path.clone()));
                }
                resp.context_menu(|ui| super::file_menu(ui, path, acts));
                x += TILE.x + 12.0;
            }
            if g.paths.len() > max_tiles {
                ui.painter().text(pos2(x, y + TILE.y / 2.0), Align2::LEFT_CENTER, format!("+{} more", g.paths.len() - max_tiles), fm(13.0), p.muted);
            }
        }
    });
    if let Some((gi, ti)) = set_keep {
        if let Some(g) = app.dups.get_mut(gi) {
            g.keep = ti;
        }
        app.refresh_cards();
    }
}
