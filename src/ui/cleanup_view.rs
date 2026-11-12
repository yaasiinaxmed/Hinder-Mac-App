//! Clean Up overview and the per-card review page.

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, Layout, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use egui_phosphor::regular as ph;

use super::Act;
use super::cards::*;
use super::widgets::*;
use crate::app::{HinderApp, View};
use crate::cleanup::CardKind;
use crate::model::{fmt_count, fmt_date_short, fmt_size, plural};
use crate::theme::{f, fb, fm, mix};

pub fn show(app: &mut HinderApp, ui: &mut Ui, acts: &mut Vec<Act>) {
    let p = app.p;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(4.0);
        ui.label(RichText::new("Clean Up").font(fb(28.0)).color(p.text));
        let total = app.cleanup_total();
        ui.label(RichText::new(if total > 0 { format!("You could free up to {}", fmt_size(total)) } else { "Nothing to clean up right now".into() }).color(p.muted));
        ui.add_space(22.0);

        let gap = 18.0;
        let (cols, w) = grid_dims(ui.available_width(), 250.0, gap, 3);
        let biggest = app.cards.iter().map(|c| c.bytes).max().unwrap_or(1).max(1);
        let cards = app.cards.clone();
        for row in cards.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in row {
                    let r = cleanup_card(ui, &p, &mut app.thumbs, c, c.bytes as f32 / biggest as f32, vec2(w, 150.0));
                    if r.clicked() && !c.paths.is_empty() {
                        acts.push(Act::Go(if c.kind == CardKind::Duplicates { View::Duplicates } else { View::Review(c.kind) }));
                    }
                }
            });
            ui.add_space(gap - 6.0);
        }
        ui.add_space(16.0);
        ui.label(RichText::new(format!("{}  Nothing is deleted without asking. Files go to the Trash, so you can always put them back.", ph::SHIELD_CHECK)).color(p.faint).font(f(12.5)));
    });
}

pub fn review(app: &mut HinderApp, ui: &mut Ui, kind: CardKind, acts: &mut Vec<Act>) {
    let p = app.p;
    let Some(card) = app.card(kind).cloned() else { return };
    if app.review_for != Some(kind) {
        app.review_for = Some(kind);
        app.review_sel = card.paths.iter().cloned().collect();
    }
    app.review_sel.retain(|x| card.paths.contains(x));

    ui.add_space(4.0);
    let back = ui.add(egui::Label::new(RichText::new(format!("{}  Clean Up", ph::CARET_LEFT)).color(p.muted)).sense(Sense::click()));
    if back.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        acts.push(Act::Go(View::CleanUp));
    }
    ui.add_space(6.0);
    ui.label(RichText::new(kind.title()).font(fb(28.0)).color(p.text));
    ui.label(RichText::new(kind.explain()).color(p.muted));
    ui.add_space(14.0);

    let sel_bytes: u64 = card.paths.iter().zip(&card.sizes).filter(|(x, _)| app.review_sel.contains(*x)).map(|(_, s)| s).sum();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} · {}", plural(card.paths.len(), "item", "items"), fmt_size(card.bytes))).font(fm(13.5)).color(p.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let n = app.review_sel.len();
            let label = if n == card.paths.len() { format!("Trash All · {}", fmt_size(sel_bytes)) } else { format!("Trash Selected ({}) · {}", fmt_count(n), fmt_size(sel_bytes)) };
            let enabled = n > 0;
            let fill = if enabled { p.red } else { mix(p.surface, p.red, 0.35) };
            if solid_button(ui, fill, Color32::WHITE, Some(ph::TRASH), &label).clicked() && enabled {
                acts.push(Act::Trash(card.paths.iter().filter(|x| app.review_sel.contains(*x)).cloned().collect()));
            }
            if outline_button(ui, &p, None, "Select None").clicked() {
                app.review_sel.clear();
            }
            if outline_button(ui, &p, None, "Select All").clicked() {
                app.review_sel = card.paths.iter().cloned().collect();
            }
        });
    });
    ui.add_space(12.0);

    let row_h = 52.0;
    ui.spacing_mut().item_spacing.y = 4.0;
    ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, card.paths.len(), |ui, range| {
        for i in range {
            let path = &card.paths[i];
            let Some(file) = app.find(path).cloned() else { continue };
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
            let checked = app.review_sel.contains(path);
            ui.painter().rect(rect, CornerRadius::same(10), if resp.hovered() { p.hover } else { p.surface }, Stroke::new(1.0, p.border), StrokeKind::Inside);
            // checkbox
            let cb = egui::Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), vec2(18.0, 18.0));
            if checked {
                ui.painter().rect_filled(cb, CornerRadius::same(5), p.accent);
                ui.painter().text(cb.center(), Align2::CENTER_CENTER, ph::CHECK, fb(12.0), p.on_accent);
            } else {
                ui.painter().rect_stroke(cb, CornerRadius::same(5), Stroke::new(1.5, p.border_strong), StrokeKind::Inside);
            }
            let t = egui::Rect::from_min_size(pos2(cb.right() + 14.0, rect.center().y - 17.0), vec2(34.0, 34.0));
            match app.thumbs.get(path) {
                Some(tex) => image_cover(ui.painter(), tex, t, CornerRadius::same(6)),
                None => {
                    ui.painter().rect_filled(t, CornerRadius::same(6), p.thumb_bg);
                    ui.painter().text(t.center(), Align2::CENTER_CENTER, file.kind.icon(), f(16.0), p.muted);
                }
            }
            let x = t.right() + 12.0;
            let name_w = rect.width() * 0.55;
            ui.painter().text(pos2(x, rect.center().y - 8.0), Align2::LEFT_CENTER, middle_truncate(ui, &file.name, &fm(13.5), name_w), fm(13.5), p.text);
            ui.painter().text(pos2(x, rect.center().y + 10.0), Align2::LEFT_CENTER, end_truncate(ui, &file.parent_display(), &f(12.0), name_w), f(12.0), p.faint);
            ui.painter().text(pos2(rect.right() - 16.0, rect.center().y - 8.0), Align2::RIGHT_CENTER, fmt_size(file.size), fm(13.0), p.text);
            let when = match file.last_used {
                Some(t) => format!("Opened {}", fmt_date_short(t)),
                None => format!("Added {}", fmt_date_short(file.date())),
            };
            ui.painter().text(pos2(rect.right() - 16.0, rect.center().y + 10.0), Align2::RIGHT_CENTER, when, f(12.0), p.faint);
            if resp.clicked() {
                if checked {
                    app.review_sel.remove(path);
                } else {
                    app.review_sel.insert(path.clone());
                }
            }
            if resp.double_clicked() {
                acts.push(Act::QuickLook(path.clone()));
            }
            resp.context_menu(|ui| super::file_menu(ui, path, acts));
        }
    });
}
