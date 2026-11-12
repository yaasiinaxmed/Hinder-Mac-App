//! "Downloaded From": every download grouped by where it came from.

use eframe::egui::{RichText, ScrollArea, Ui, vec2};
use egui_phosphor::regular as ph;

use super::Act;
use super::cards::*;
use crate::app::{HinderApp, View};
use crate::model::{fmt_count, plural};
use crate::theme::{f, fb, fm};

pub fn show(app: &mut HinderApp, ui: &mut Ui, acts: &mut Vec<Act>) {
    let p = app.p;
    ui.add_space(4.0);
    ui.label(RichText::new("Downloaded From").font(fb(28.0)).color(p.text));
    let files: usize = app.sources.iter().map(|s| s.count).sum();
    ui.label(RichText::new(format!("{} downloaded files from {}", fmt_count(files), plural(app.sources.len(), "source", "sources"))).color(p.muted));
    ui.add_space(20.0);

    if app.sources.is_empty() {
        ui.add_space(50.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(ph::DOWNLOAD_SIMPLE).font(f(44.0)).color(p.faint));
            ui.add_space(8.0);
            ui.label(RichText::new("No download sources found").font(fm(17.0)).color(p.text));
            ui.label(RichText::new("macOS records where a file came from when a browser, Mail or a chat app saves it.").color(p.muted));
        });
        return;
    }

    let gap = 14.0;
    let (cols, w) = grid_dims(ui.available_width(), 230.0, gap, 6);
    let rows = app.sources.len().div_ceil(cols);
    ui.spacing_mut().item_spacing.y = gap;
    ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, 76.0, rows, |ui, range| {
        for r in range {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in 0..cols {
                    let Some(s) = app.sources.get(r * cols + c) else { break };
                    let resp = source_card(ui, &p, &mut app.thumbs, s, vec2(w, 76.0));
                    let key = s.key.clone();
                    if resp.clicked() {
                        acts.push(Act::Go(View::Source(key)));
                    }
                }
            });
        }
    });
}
