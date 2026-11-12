//! Any list of files: shelves, smart folders, locations, sources, search results.

use eframe::egui::{Align, Layout, RichText, ScrollArea, Ui, vec2};
use egui_phosphor::regular as ph;

use super::Act;
use super::cards::*;
use super::widgets::*;
use crate::app::{HinderApp, Sort, View};
use crate::model::{Kind, fmt_count, fmt_size, tildify};
use crate::query::{DateFilter, SizeFilter};
use crate::theme::{Palette, f, fb, fm};

const GAP: f32 = 18.0;

#[derive(Clone, PartialEq)]
enum KindOpt {
    All,
    Kind(Kind),
    Pdf,
    Screenshots,
    Custom,
}

fn title_for(app: &HinderApp) -> String {
    match &app.view {
        View::Recents => "Recents".into(),
        View::Kind(k) => k.label().into(),
        View::LargeFiles => "Large Files".into(),
        View::Screenshots => "Screenshots".into(),
        View::Source(k) => app.source_label(k),
        View::Folder(p) => p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| tildify(p)),
        View::Search => format!("Results for “{}”", app.search_text.trim()),
        _ => String::new(),
    }
}

pub fn show(app: &mut HinderApp, ui: &mut Ui, acts: &mut Vec<Act>) {
    let p = app.p;
    let idx = app.visible();
    let total: u64 = idx.iter().map(|&i| app.lib.files[i].size).sum();

    // ---------------- header
    ui.add_space(4.0);
    let title = title_for(app);
    let title = end_truncate(ui, &title, &fb(28.0), ui.available_width() - 20.0);
    ui.label(RichText::new(title).font(fb(28.0)).color(p.text));
    let sub = match &app.view {
        View::Search => {
            let searching = !app.filters.text.is_empty() && app.content_hits.is_none();
            format!(
                "{} in your folders{}",
                crate::model::plural(idx.len(), "match", "matches"),
                if searching { " · searching inside files…" } else { "" }
            )
        }
        View::Folder(p) => format!("{} · {} · {}", crate::model::plural(idx.len(), "item", "items"), fmt_size(total), tildify(p)),
        View::LargeFiles => format!("{} over 500 MB · {}", crate::model::plural(idx.len(), "file", "files"), fmt_size(total)),
        _ => format!("{} · {}", crate::model::plural(idx.len(), "item", "items"), fmt_size(total)),
    };
    ui.label(RichText::new(sub).color(p.muted));
    ui.add_space(14.0);

    filter_bar(app, ui);

    // "Understood as" chips
    if app.view == View::Search {
        let chips = app.filters.chips(|k| app.source_label(k));
        if !chips.is_empty() {
            ui.add_space(10.0);
            let mut remove = None;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{}  Understood as", ph::SPARKLE)).color(p.muted).font(f(13.0)));
                ui.add_space(2.0);
                for (i, c) in chips.iter().enumerate() {
                    if chip(ui, &p, &c.label, i) {
                        remove = Some(c.facet.clone());
                    }
                }
            });
            if let Some(fct) = remove {
                app.filters.remove(&fct);
            }
        }
    }
    ui.add_space(18.0);

    if idx.is_empty() {
        empty_state(ui, &p, app.view == View::Search);
        return;
    }
    section_title(ui, &p, "Files", Some(fmt_count(idx.len())));
    ui.add_space(10.0);

    if app.prefs.grid {
        grid(app, ui, &idx, acts);
    } else {
        list(app, ui, &idx, acts);
    }
}

fn filter_bar(app: &mut HinderApp, ui: &mut Ui) {
    let p = app.p;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        // From
        if !app.sources.is_empty() && !matches!(app.view, View::Source(_)) {
            let mut opts: Vec<(String, Option<String>)> = vec![("Anywhere".into(), None)];
            opts.extend(app.sources.iter().take(40).map(|s| (format!("{}  ({})", s.label, s.count), Some(s.key.clone()))));
            let current = app.filters.source.as_ref().map(|k| app.source_label(k)).unwrap_or("Anywhere".into());
            let mut v = app.filters.source.clone();
            if dropdown(ui, &p, "dd-from", "From", &current, &opts, &mut v) {
                app.filters.source = v;
            }
        }
        // Kind
        if !matches!(app.view, View::Kind(_) | View::Screenshots) {
            let f = &app.filters;
            let cur = if f.screenshots && f.kinds.is_empty() && f.exts.is_empty() {
                KindOpt::Screenshots
            } else if f.kinds.is_empty() && f.exts.len() == 1 && f.exts.contains("pdf") {
                KindOpt::Pdf
            } else if f.kinds.len() == 1 && f.exts.is_empty() && !f.screenshots {
                KindOpt::Kind(*f.kinds.iter().next().unwrap())
            } else if f.kinds.is_empty() && f.exts.is_empty() && !f.screenshots {
                KindOpt::All
            } else {
                KindOpt::Custom
            };
            let mut opts: Vec<(String, KindOpt)> = vec![("All".into(), KindOpt::All)];
            opts.extend(Kind::SHELVES.iter().map(|k| (k.label().to_string(), KindOpt::Kind(*k))));
            opts.push(("PDFs".into(), KindOpt::Pdf));
            opts.push(("Screenshots".into(), KindOpt::Screenshots));
            let label = match &cur {
                KindOpt::All => "All".to_string(),
                KindOpt::Kind(k) => k.label().into(),
                KindOpt::Pdf => "PDFs".into(),
                KindOpt::Screenshots => "Screenshots".into(),
                KindOpt::Custom => "Mixed".into(),
            };
            let mut v = cur.clone();
            if dropdown(ui, &p, "dd-kind", "Kind", &label, &opts, &mut v) {
                let q = &mut app.filters;
                q.kinds.clear();
                q.exts.clear();
                q.screenshots = false;
                match v {
                    KindOpt::Kind(k) => {
                        q.kinds.insert(k);
                    }
                    KindOpt::Pdf => {
                        q.exts.insert("pdf".into());
                    }
                    KindOpt::Screenshots => q.screenshots = true,
                    _ => {}
                }
            }
        }
        // Date
        let mut dopts: Vec<(String, DateFilter)> = DateFilter::PRESETS.iter().map(|d| (d.label(), *d)).collect();
        if !DateFilter::PRESETS.contains(&app.filters.date) {
            dopts.push((app.filters.date.label(), app.filters.date));
        }
        let mut d = app.filters.date;
        if dropdown(ui, &p, "dd-date", "Date", &app.filters.date.label(), &dopts, &mut d) {
            app.filters.date = d;
        }
        // Size
        let mut sopts: Vec<(String, SizeFilter)> = SizeFilter::PRESETS.iter().map(|(l, s)| (l.to_string(), *s)).collect();
        if !SizeFilter::PRESETS.iter().any(|(_, s)| *s == app.filters.size) {
            sopts.push((app.filters.size.label(), app.filters.size));
        }
        let mut s = app.filters.size;
        if dropdown(ui, &p, "dd-size", "Size", &app.filters.size.label(), &sopts, &mut s) {
            app.filters.size = s;
        }
        // Sort
        let sort_opts: Vec<(String, Sort)> = Sort::ALL.iter().map(|s| (s.label().to_string(), *s)).collect();
        let mut so = app.prefs.sort;
        if dropdown(ui, &p, "dd-sort", "Sort", app.prefs.sort.label(), &sort_opts, &mut so) {
            app.prefs.sort = so;
        }
        ui.add_space(6.0);
        if icon_button(ui, &p, ph::SQUARES_FOUR, app.prefs.grid, "Grid  ⌘1").clicked() {
            app.prefs.grid = true;
        }
        if icon_button(ui, &p, ph::LIST, !app.prefs.grid, "List  ⌘2").clicked() {
            app.prefs.grid = false;
        }
        if app.prefs.grid {
            if icon_button(ui, &p, ph::MINUS, false, "Smaller cards  ⌘−").clicked() {
                app.prefs.zoom = (app.prefs.zoom - 0.15).max(0.6);
            }
            if icon_button(ui, &p, ph::PLUS, false, "Bigger cards  ⌘+").clicked() {
                app.prefs.zoom = (app.prefs.zoom + 0.15).min(2.0);
            }
        }
    });
}

fn apply_nav(app: &mut HinderApp, idx: &[usize], cols: usize) -> Option<usize> {
    let (dx, dy) = app.nav_delta.take()?;
    let cur = app.selected.as_ref().and_then(|s| idx.iter().position(|&i| app.lib.files[i].path == *s));
    let next = match cur {
        None => 0,
        Some(c) => (c as i64 + dx + dy * cols as i64).clamp(0, idx.len() as i64 - 1) as usize,
    };
    app.selected = Some(app.lib.files[idx[next]].path.clone());
    Some(next)
}

fn grid(app: &mut HinderApp, ui: &mut Ui, idx: &[usize], acts: &mut Vec<Act>) {
    let p = app.p;
    let avail = ui.available_width();
    let (cols, w) = grid_dims(avail, 220.0 * app.prefs.zoom, GAP, 12);
    app.grid_cols = cols;
    let card_h = w * 0.68 + 58.0;
    let rows = idx.len().div_ceil(cols);
    let moved = apply_nav(app, idx, cols);

    let mut area = ScrollArea::vertical().auto_shrink([false, false]).id_salt(format!("grid-{:?}", app.view));
    if let Some(m) = moved {
        let row = m / cols;
        let (first, last) = app.last_rows;
        if row < first || row >= last {
            area = area.vertical_scroll_offset((row as f32 * (card_h + GAP) - GAP).max(0.0));
        }
    }
    ui.spacing_mut().item_spacing.y = GAP;
    let out = area.show_rows(ui, card_h, rows, |ui, range| {
        let first = range.start;
        let last = range.end;
        for r in range {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                for c in 0..cols {
                    let Some(&i) = idx.get(r * cols + c) else { break };
                    let file = &app.lib.files[i];
                    let sel = app.selected.as_ref() == Some(&file.path);
                    let resp = file_card(ui, &p, &mut app.thumbs, file, vec2(w, card_h), sel);
                    let path = file.path.clone();
                    if resp.clicked() {
                        acts.push(Act::Select(path.clone()));
                    }
                    if resp.double_clicked() {
                        acts.push(Act::Open(path.clone()));
                    }
                    resp.context_menu(|ui| super::file_menu(ui, &path, acts));
                }
            });
        }
        (first, last)
    });
    app.last_rows = out.inner;
}

fn list(app: &mut HinderApp, ui: &mut Ui, idx: &[usize], acts: &mut Vec<Act>) {
    let p = app.p;
    list_header(ui, &p);
    let row_h = 40.0;
    let moved = apply_nav(app, idx, 1);
    let mut area = ScrollArea::vertical().auto_shrink([false, false]).id_salt(format!("list-{:?}", app.view));
    if let Some(m) = moved {
        let (first, last) = app.last_rows;
        if m < first || m + 1 >= last {
            area = area.vertical_scroll_offset((m as f32 * (row_h + 2.0) - 80.0).max(0.0));
        }
    }
    ui.spacing_mut().item_spacing.y = 2.0;
    let out = area.show_rows(ui, row_h, idx.len(), |ui, range| {
        let (first, last) = (range.start, range.end);
        for r in range {
            let file = &app.lib.files[idx[r]];
            let sel = app.selected.as_ref() == Some(&file.path);
            let resp = file_row(ui, &p, &mut app.thumbs, file, row_h, sel, r % 2 == 1);
            let path = file.path.clone();
            if resp.clicked() {
                acts.push(Act::Select(path.clone()));
            }
            if resp.double_clicked() {
                acts.push(Act::Open(path.clone()));
            }
            resp.context_menu(|ui| super::file_menu(ui, &path, acts));
        }
        (first, last)
    });
    app.last_rows = out.inner;
}

fn empty_state(ui: &mut Ui, p: &Palette, search: bool) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(if search { ph::MAGNIFYING_GLASS } else { ph::FOLDER_DASHED }).font(f(44.0)).color(p.faint));
        ui.add_space(10.0);
        ui.label(RichText::new(if search { "No matches" } else { "Nothing here yet" }).font(fm(17.0)).color(p.text));
        ui.add_space(4.0);
        let hint = if search {
            "Try fewer words, or remove a filter chip above. Examples: “screenshots from yesterday”, “zips bigger than 1 gb”."
        } else {
            "Files you add will show up here."
        };
        ui.label(RichText::new(hint).color(p.muted));
    });
    let _ = Layout::top_down(Align::Center);
}
