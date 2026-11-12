//! Home dashboard: greeting, library shelves, Clean Up, recent files, sources.

use chrono::Timelike;
use eframe::egui::{self, Align, Layout, RichText, ScrollArea, Sense, Ui, vec2};
use egui_phosphor::regular as ph;

use super::Act;
use super::cards::*;
use super::widgets::*;
use crate::app::{HinderApp, View};
use crate::model::{Kind, fmt_count, fmt_size};
use crate::theme::{Look, f, fb, fm};

const GAP: f32 = 18.0;

fn greeting() -> &'static str {
    match chrono::Local::now().hour() {
        5..=11 => "Good morning",
        12..=16 => "Good afternoon",
        17..=21 => "Good evening",
        _ => "Hello, night owl",
    }
}

fn see_all(ui: &mut Ui, p: &crate::theme::Palette) -> bool {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let r = ui.add(egui::Label::new(RichText::new(format!("See all {}", ph::ARROW_RIGHT)).color(p.muted).font(f(13.0))).sense(Sense::click()));
        r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    })
    .inner
}

pub fn show(app: &mut HinderApp, ui: &mut Ui, acts: &mut Vec<Act>) {
    let p = app.p;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(4.0);
        // ---- greeting
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(greeting()).font(fb(30.0)).color(p.text));
                ui.add_space(2.0);
                ui.label(RichText::new(format!("{} files · {} across your Mac", fmt_count(app.lib.files.len()), fmt_size(app.lib.total_size()))).color(p.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if app.scanning {
                    ui.add(egui::Spinner::new().size(16.0).color(p.muted));
                    ui.label(RichText::new("Refreshing…").color(p.muted));
                } else if outline_button(ui, &p, Some(ph::ARROW_CLOCKWISE), "Refresh").on_hover_text("Rescan  ⌘R").clicked() {
                    app.rescan(ui.ctx());
                }
            });
        });
        ui.add_space(26.0);

        let avail = ui.available_width();

        // ---- library shelves
        section_title(ui, &p, "Library", Some("6".into()));
        ui.add_space(12.0);
        let (cols, w) = grid_dims(avail, 230.0, GAP, 6);
        let shelves: Vec<Kind> = Kind::SHELVES.to_vec();
        for row in shelves.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                for &k in row {
                    let (count, bytes) = app.stats.get(&k).copied().unwrap_or((0, 0));
                    let samples = app.shelf_samples.get(&k).cloned().unwrap_or_default();
                    if shelf_card(ui, &p, &mut app.thumbs, k, count, bytes, &samples, vec2(w, 196.0)).clicked() {
                        acts.push(Act::Go(View::Kind(k)));
                    }
                }
            });
            ui.add_space(GAP - 6.0);
        }
        ui.add_space(20.0);

        // ---- clean up
        let total = app.cleanup_total();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Clean Up").font(fm(20.0)).color(p.text));
            ui.add_space(4.0);
            if total > 0 {
                let (fill, col) = if p.look == Look::Vivid { (p.green, egui::Color32::WHITE) } else { (p.text, p.bg) };
                pill(ui, &format!("Free up to {}", fmt_size(total)), fill, col);
            }
            if see_all(ui, &p) {
                acts.push(Act::Go(View::CleanUp));
            }
        });
        ui.add_space(12.0);
        let (cols, w) = grid_dims(avail, 230.0, GAP, 4);
        let mut cards: Vec<_> = app.cards.iter().filter(|c| !c.paths.is_empty()).cloned().collect();
        cards.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        if cards.is_empty() {
            ui.label(RichText::new(if app.dups_ready { "Nothing to clean up. Nice and tidy." } else { "Looking for things to clean up…" }).color(p.muted));
        }
        let biggest = cards.first().map(|c| c.bytes).unwrap_or(1).max(1);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for c in cards.iter().take(cols) {
                if cleanup_card(ui, &p, &mut app.thumbs, c, c.bytes as f32 / biggest as f32, vec2(w, 150.0)).clicked() {
                    acts.push(Act::Go(if c.kind == crate::cleanup::CardKind::Duplicates { View::Duplicates } else { View::Review(c.kind) }));
                }
            }
        });
        ui.add_space(32.0);

        // ---- recents shelf
        let recent = app.recent.clone();
        if !recent.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Recent").font(fm(20.0)).color(p.text));
                if see_all(ui, &p) {
                    acts.push(Act::Go(View::Recents));
                }
            });
            ui.add_space(12.0);
            let (cols, w) = grid_dims(avail, 200.0, GAP, 6);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                for &i in recent.iter().take(cols) {
                    let file = app.lib.files[i].clone();
                    let sel = app.selected.as_ref() == Some(&file.path);
                    let r = file_card(ui, &p, &mut app.thumbs, &file, vec2(w, w * 0.62 + 58.0), sel);
                    if r.clicked() {
                        acts.push(Act::Select(file.path.clone()));
                    }
                    if r.double_clicked() {
                        acts.push(Act::Open(file.path.clone()));
                    }
                    r.context_menu(|ui| super::file_menu(ui, &file.path, acts));
                }
            });
            ui.add_space(32.0);
        }

        // ---- sources
        if !app.sources.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Downloaded From").font(fm(20.0)).color(p.text));
                ui.add_space(4.0);
                pill(ui, &fmt_count(app.sources.len()), p.thumb_bg, p.muted);
                if see_all(ui, &p) {
                    acts.push(Act::Go(View::Sources));
                }
            });
            ui.add_space(12.0);
            let (cols, w) = grid_dims(avail, 220.0, 14.0, 5);
            let n = (cols * 2).min(app.sources.len());
            for row in (0..n).collect::<Vec<_>>().chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    for &i in row {
                        let s = &app.sources[i];
                        if source_card(ui, &p, &mut app.thumbs, s, vec2(w, 74.0)).clicked() {
                            acts.push(Act::Go(View::Source(s.key.clone())));
                        }
                    }
                });
                ui.add_space(8.0);
            }
        }
        ui.add_space(40.0);
    });
}
