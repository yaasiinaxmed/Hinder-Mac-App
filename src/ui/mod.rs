//! The window: sidebar, top bar, main content, inspector, dialogs.

pub mod cards;
pub mod cleanup_view;
pub mod dupes_view;
pub mod files;
pub mod home;
pub mod sources_view;
pub mod widgets;

use std::path::PathBuf;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Frame, Id, Layout, Margin, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, pos2, vec2,
};
use egui_phosphor::regular as ph;

use crate::app::{HinderApp, View};
use crate::model::{Kind, fmt_count, fmt_date, fmt_size, home_dir, tildify};
use crate::theme::{Look, Palette, f, fb, fm};
use crate::{actions, sources};
use widgets::*;

/// Deferred UI actions, applied after drawing so widgets don't fight over `&mut app`.
pub enum Act {
    Select(PathBuf),
    Open(PathBuf),
    QuickLook(PathBuf),
    Reveal(PathBuf),
    CopyPath(PathBuf),
    Pin(PathBuf),
    Unpin(PathBuf),
    Trash(Vec<PathBuf>),
    Go(View),
}

pub fn draw(app: &mut HinderApp, ctx: &egui::Context, now: f64) {
    let mut acts: Vec<Act> = Vec::new();

    if app.prefs.sidebar {
        sidebar(app, ctx, &mut acts);
    }
    topbar(app, ctx, now);
    if app.prefs.inspector {
        inspector(app, ctx, &mut acts);
    }

    let p = app.p;
    egui::CentralPanel::default()
        .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 36, right: 36, top: 24, bottom: 0 }))
        .show(ctx, |ui| {
            if app.lib.files.is_empty() && app.scanning {
                indexing(ui, &p, app.logo.as_ref(), app.scan_progress);
                return;
            }
            match app.view.clone() {
                View::Home => home::show(app, ui, &mut acts),
                View::Sources => sources_view::show(app, ui, &mut acts),
                View::Duplicates => dupes_view::show(app, ui, &mut acts),
                View::CleanUp => cleanup_view::show(app, ui, &mut acts),
                View::Review(k) => cleanup_view::review(app, ui, k, &mut acts),
                _ => files::show(app, ui, &mut acts),
            }
        });

    confirm_dialog(app, ctx, now);
    toast(app, ctx);

    for a in acts {
        match a {
            Act::Select(p) => app.selected = Some(p),
            Act::Open(p) => actions::open(&p),
            Act::QuickLook(p) => actions::quick_look(&[p]),
            Act::Reveal(p) => actions::reveal_in_finder(&p),
            Act::CopyPath(p) => ctx.copy_text(p.display().to_string()),
            Act::Pin(p) => {
                let dir = if p.is_dir() && p.extension().is_none_or(|e| e != "app") { p } else { p.parent().map(|x| x.to_path_buf()).unwrap_or(p) };
                if !app.prefs.pinned.contains(&dir) {
                    app.prefs.pinned.push(dir);
                }
            }
            Act::Unpin(p) => app.prefs.pinned.retain(|x| *x != p),
            Act::Trash(paths) => app.ask_trash(paths),
            Act::Go(v) => app.go(v),
        }
    }
}

/// The standard right-click menu for a file.
pub fn file_menu(ui: &mut Ui, path: &std::path::Path, acts: &mut Vec<Act>) {
    let pb = path.to_path_buf();
    if ui.button(format!("{}  Quick Look", ph::EYE)).clicked() {
        acts.push(Act::QuickLook(pb.clone()));
        ui.close_menu();
    }
    if ui.button(format!("{}  Open", ph::ARROW_SQUARE_OUT)).clicked() {
        acts.push(Act::Open(pb.clone()));
        ui.close_menu();
    }
    if ui.button(format!("{}  Show in Finder", ph::FOLDER_OPEN)).clicked() {
        acts.push(Act::Reveal(pb.clone()));
        ui.close_menu();
    }
    ui.separator();
    if ui.button(format!("{}  Copy Path", ph::COPY)).clicked() {
        acts.push(Act::CopyPath(pb.clone()));
        ui.close_menu();
    }
    if ui.button(format!("{}  Pin Folder to Sidebar", ph::PUSH_PIN)).clicked() {
        acts.push(Act::Pin(pb.clone()));
        ui.close_menu();
    }
    ui.separator();
    if ui.button(RichText::new(format!("{}  Move to Trash", ph::TRASH)).color(Color32::from_rgb(0xE5, 0x48, 0x4D))).clicked() {
        acts.push(Act::Trash(vec![pb]));
        ui.close_menu();
    }
}

fn window_drag(ctx: &egui::Context, resp: &egui::Response) {
    if resp.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
    }
}

// ------------------------------------------------------------------ sidebar

enum Trail {
    None,
    Muted(String),
    Strong(String, Color32),
    Pill(String, Color32, Color32),
}

fn nav_item(ui: &mut Ui, p: &Palette, icon: &str, icon_color: Color32, label: &str, trail: Trail, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    let fill = if selected { p.selected } else if resp.hovered() { p.hover } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().text(pos2(rect.left() + 18.0, rect.center().y), Align2::CENTER_CENTER, icon, f(16.0), icon_color);
    let label_font = if selected { fm(13.5) } else { f(13.5) };
    let label_w = rect.width() - 100.0;
    let shown = end_truncate(ui, label, &label_font, label_w);
    ui.painter().text(pos2(rect.left() + 36.0, rect.center().y), Align2::LEFT_CENTER, shown, label_font, p.text);
    let right = pos2(rect.right() - 10.0, rect.center().y);
    match trail {
        Trail::None => {}
        Trail::Muted(s) => {
            ui.painter().text(right, Align2::RIGHT_CENTER, s, f(12.0), p.faint);
        }
        Trail::Strong(s, c) => {
            ui.painter().text(right, Align2::RIGHT_CENTER, s, fm(12.0), c);
        }
        Trail::Pill(s, fill, color) => {
            let w = text_width(ui, &s, &fm(11.5)) + 14.0;
            let r = egui::Rect::from_min_size(pos2(right.x - w, right.y - 10.0), vec2(w, 20.0));
            ui.painter().rect_filled(r, CornerRadius::same(10), fill);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, s, fm(11.5), color);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn sidebar(app: &mut HinderApp, ctx: &egui::Context, acts: &mut Vec<Act>) {
    let p = app.p;
    egui::SidePanel::left("sidebar")
        .exact_width(240.0)
        .resizable(false)
        .frame(Frame::new().fill(p.sidebar).inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 8 }).stroke(p.hairline()))
        .show(ctx, |ui| {
            // Room for the traffic lights; also lets you drag the window.
            let (_, drag) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click_and_drag());
            window_drag(ctx, &drag);

            ui.horizontal(|ui| {
                ui.add_space(6.0);
                let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                logo(ui.painter(), app.logo.as_ref(), r);
                ui.add_space(4.0);
                ui.label(RichText::new("Hinder").font(fb(19.0)).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, &p, ph::SIDEBAR_SIMPLE, false, "Hide sidebar").clicked() {
                        app.prefs.sidebar = false;
                    }
                });
            });
            ui.add_space(14.0);

            let icon_c = |slot: usize| if p.colorful() { p.hue(slot) } else { p.muted };
            // Scrolls with the trackpad, but never shows a bar (like Finder's sidebar).
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if nav_item(ui, &p, ph::HOUSE, p.muted, "Home", Trail::None, app.view == View::Home).clicked() {
                    acts.push(Act::Go(View::Home));
                }
                if nav_item(ui, &p, ph::CLOCK, p.muted, "Recents", Trail::None, app.view == View::Recents).clicked() {
                    acts.push(Act::Go(View::Recents));
                }
                for k in Kind::SHELVES {
                    let n = app.stats.get(&k).map(|s| s.0).unwrap_or(0);
                    let col = icon_c(Palette::kind_slot(k));
                    // In folder mode the shelf opens on folders, so count those (with the
                    // file total on hover); otherwise count files.
                    let folders = (app.prefs.kind_folders && HinderApp::can_browse(&View::Kind(k))).then(|| app.shelf_folder_count(k)).filter(|&c| c > 0);
                    let trail = match folders {
                        Some(c) => Trail::Muted(format!("{} {}", ph::FOLDER_SIMPLE, fmt_count(c))),
                        None => Trail::Muted(fmt_count(n)),
                    };
                    let r = nav_item(ui, &p, k.icon(), col, k.label(), trail, *app.base_view() == View::Kind(k));
                    let r = match folders {
                        Some(c) => r.on_hover_text(format!(
                            "{} in {}",
                            crate::model::plural(n, &k.label().to_lowercase(), &k.label().to_lowercase()),
                            crate::model::plural(c, "folder", "folders")
                        )),
                        None => r,
                    };
                    if r.clicked() {
                        acts.push(Act::Go(View::Kind(k)));
                    }
                }
                divider(ui, &p);

                let large = app.large_count();
                let large_trail = if large > 0 { Trail::Pill(large.to_string(), p.accent, p.on_accent) } else { Trail::Muted("0".into()) };
                if nav_item(ui, &p, ph::HARD_DRIVES, p.muted, "Large Files", large_trail, *app.base_view() == View::LargeFiles).clicked() {
                    acts.push(Act::Go(View::LargeFiles));
                }
                if nav_item(ui, &p, ph::SCAN, p.muted, "Screenshots", Trail::Muted(fmt_count(app.screenshot_count())), *app.base_view() == View::Screenshots).clicked() {
                    acts.push(Act::Go(View::Screenshots));
                }
                let dup_trail = if !app.dups_ready {
                    Trail::Muted("…".into())
                } else {
                    let b = crate::dupes::total_reclaimable(&app.dups);
                    if b > 0 { Trail::Strong(fmt_size(b), p.text) } else { Trail::Muted("—".into()) }
                };
                if nav_item(ui, &p, ph::COPY, p.muted, "Duplicates", dup_trail, app.view == View::Duplicates).clicked() {
                    acts.push(Act::Go(View::Duplicates));
                }
                let total = app.cleanup_total();
                let clean_col = if p.colorful() { p.green } else { p.text };
                if nav_item(ui, &p, ph::SPARKLE, p.muted, "Clean Up", Trail::Strong(fmt_size(total), clean_col), matches!(app.view, View::CleanUp | View::Review(_))).clicked() {
                    acts.push(Act::Go(View::CleanUp));
                }
                if nav_item(ui, &p, ph::DOWNLOAD_SIMPLE, p.muted, "Downloaded From", Trail::Muted(fmt_count(app.sources.len())), matches!(app.base_view(), View::Sources | View::Source(_))).clicked() {
                    acts.push(Act::Go(View::Sources));
                }
                divider(ui, &p);

                group_label(ui, &p, "Locations");
                let home = home_dir();
                let folder_icon_col = if p.colorful() { p.hue(1) } else { p.muted };
                let mut locs = vec![(ph::HOUSE_LINE, home.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or("Home".into()), home.clone())];
                for d in ["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Music"] {
                    let path = home.join(d);
                    if path.is_dir() {
                        let icon = match d {
                            "Desktop" => ph::DESKTOP,
                            "Downloads" => ph::DOWNLOAD_SIMPLE,
                            _ => ph::FOLDER_SIMPLE,
                        };
                        locs.push((icon, d.to_string(), path));
                    }
                }
                for (icon, label, path) in locs {
                    let sel = app.view == View::Folder(path.clone());
                    let r = nav_item(ui, &p, icon, folder_icon_col, &label, Trail::None, sel);
                    if r.clicked() {
                        acts.push(Act::Go(View::Folder(path.clone())));
                    }
                    r.context_menu(|ui| {
                        if ui.button(format!("{}  Show in Finder", ph::FOLDER_OPEN)).clicked() {
                            acts.push(Act::Open(path.clone()));
                            ui.close_menu();
                        }
                    });
                }
                if !app.prefs.pinned.is_empty() {
                    divider(ui, &p);
                    group_label(ui, &p, "Pinned");
                    for path in app.prefs.pinned.clone() {
                        let label = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| tildify(&path));
                        let sel = app.view == View::Folder(path.clone());
                        let r = nav_item(ui, &p, ph::PUSH_PIN, folder_icon_col, &label, Trail::None, sel).on_hover_text(tildify(&path));
                        if r.clicked() {
                            acts.push(Act::Go(View::Folder(path.clone())));
                        }
                        r.context_menu(|ui| {
                            if ui.button(format!("{}  Show in Finder", ph::FOLDER_OPEN)).clicked() {
                                acts.push(Act::Open(path.clone()));
                                ui.close_menu();
                            }
                            if ui.button(format!("{}  Unpin", ph::X)).clicked() {
                                acts.push(Act::Unpin(path.clone()));
                                ui.close_menu();
                            }
                        });
                    }
                }
                ui.add_space(12.0);
                let status = if app.scanning {
                    format!("{}  Indexing… {} files", ph::CIRCLE_NOTCH, fmt_count(app.scan_progress))
                } else if !app.dups_ready {
                    format!("{}  Looking for duplicates…", ph::CIRCLE_NOTCH)
                } else {
                    format!("{}  Private · on this Mac only", ph::SHIELD_CHECK)
                };
                ui.label(RichText::new(status).font(f(11.5)).color(p.faint));
            });
        });
}

fn divider(ui: &mut Ui, p: &Palette) {
    ui.add_space(8.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([pos2(r.left() + 10.0, r.center().y), pos2(r.right() - 10.0, r.center().y)], Stroke::new(1.0, p.border));
    ui.add_space(8.0);
}

fn group_label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(text).font(fm(11.5)).color(p.faint));
    });
    ui.add_space(2.0);
}

// ------------------------------------------------------------------ top bar

fn topbar(app: &mut HinderApp, ctx: &egui::Context, now: f64) {
    let p = app.p;
    egui::TopBottomPanel::top("topbar")
        .exact_height(64.0)
        .frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 28, right: 18, top: 0, bottom: 0 }).stroke(p.hairline()))
        .show(ctx, |ui| {
            // Empty space in the bar drags the window (registered first, so widgets win).
            let bg = ui.interact(ui.max_rect(), Id::new("topbar-drag"), Sense::click_and_drag());
            window_drag(ctx, &bg);

            ui.horizontal_centered(|ui| {
                if !app.prefs.sidebar {
                    ui.add_space(64.0); // traffic lights
                    if icon_button(ui, &p, ph::SIDEBAR_SIMPLE, false, "Show sidebar").clicked() {
                        app.prefs.sidebar = true;
                    }
                    ui.add_space(6.0);
                }
                if !app.history.is_empty() && app.view != View::Home {
                    if icon_button(ui, &p, ph::CARET_LEFT, false, "Back  ⌘[").clicked() {
                        app.back();
                    }
                    ui.add_space(4.0);
                }
                search_box(app, ui, now);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, &p, ph::SIDEBAR_SIMPLE, app.prefs.inspector, "Inspector  ⌘I").clicked() {
                        app.prefs.inspector = !app.prefs.inspector;
                    }
                    ui.add_space(8.0);
                    look_toggle(app, ui);
                    ui.add_space(4.0);
                    appearance_button(app, ui);
                    ui.add_space(12.0);
                    if ui.available_width() > 150.0 {
                        ui.label(RichText::new(fmt_size(app.lib.total_size())).font(fm(13.0)).color(p.text));
                        ui.label(RichText::new("Library size").font(f(13.0)).color(p.muted));
                    }
                });
            });
        });
}

fn search_box(app: &mut HinderApp, ui: &mut Ui, now: f64) {
    let p = app.p;
    let id = Id::new("hinder-search");
    let focused = ui.ctx().memory(|m| m.has_focus(id));
    let width = (ui.available_width() - 280.0).clamp(220.0, 460.0);
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(if focused { 1.5 } else { 1.0 }, if focused { p.border_strong } else { p.border }))
        .corner_radius(CornerRadius::same(11))
        .inner_margin(Margin { left: 12, right: 8, top: 0, bottom: 0 })
        .show(ui, |ui| {
            ui.set_width(width);
            ui.set_height(38.0);
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new(ph::MAGNIFYING_GLASS).font(f(16.0)).color(p.muted));
                let right_w = if app.search_text.is_empty() { 52.0 } else { 26.0 };
                let te = TextEdit::singleline(&mut app.search_text)
                    .id(id)
                    .frame(false)
                    .font(f(14.0))
                    .text_color(p.text)
                    .hint_text(RichText::new("Search, or try “pdfs from last week”…").color(p.faint))
                    .desired_width(ui.available_width() - right_w)
                    .margin(Margin::symmetric(2, 0));
                let r = ui.add(te);
                if app.focus_search {
                    r.request_focus();
                    app.focus_search = false;
                }
                if app.search_text.is_empty() {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    kbd(ui, &p, "⌘");
                    kbd(ui, &p, "F");
                } else {
                    let (cr, cresp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    ui.painter().text(cr.center(), Align2::CENTER_CENTER, ph::X_CIRCLE, f(17.0), if cresp.hovered() { p.text } else { p.faint });
                    if cresp.clicked() {
                        app.search_text.clear();
                    }
                }
            });
        });
    app.on_search_changed(now);
}

fn look_toggle(app: &mut HinderApp, ui: &mut Ui) {
    let p = app.p;
    // A fixed-size pill (three 30×26 segments + 3px padding). Painting it by hand
    // keeps the background from stretching across the free space in the
    // right-to-left top-bar layout.
    const SEG: egui::Vec2 = vec2(30.0, 26.0);
    const PAD: f32 = 3.0;
    let looks = [
        (Look::Vivid, ph::PALETTE, "Vivid look: Hinder copper  ⌃⌘T"),
        (Look::Normal, ph::SUN_HORIZON, "Normal look: plain macOS greys  ⌃⌘T"),
        (Look::Mono, ph::CIRCLE_HALF, "Mono look: black and white  ⌃⌘T"),
    ];
    let n = looks.len() as f32;
    let (pill, _) = ui.allocate_exact_size(vec2(SEG.x * n + PAD * 2.0, SEG.y + PAD * 2.0), Sense::hover());
    ui.painter().rect_filled(pill, CornerRadius::same(10), p.thumb_bg);
    for (i, (look, icon, tip)) in looks.into_iter().enumerate() {
        let r = egui::Rect::from_min_size(pill.min + vec2(PAD + SEG.x * i as f32, PAD), SEG);
        let resp = ui.interact(r, Id::new(("look-toggle", i)), Sense::click());
        let active = app.prefs.look == look;
        if active {
            ui.painter().rect(r, CornerRadius::same(7), p.surface, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        let color = if active || resp.hovered() { p.text } else { p.faint };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, f(15.0), color);
        if resp.on_hover_text(tip).clicked() {
            app.prefs.look = look;
        }
    }
}

/// Light / Dark / follow macOS, cycled by one button.
fn appearance_button(app: &mut HinderApp, ui: &mut Ui) {
    let p = app.p;
    let (icon, tip) = match app.prefs.appearance {
        crate::theme::Appearance::System => (ph::CIRCLE_HALF_TILT, "Appearance: follows macOS (click for Light)"),
        crate::theme::Appearance::Light => (ph::SUN, "Appearance: Light (click for Dark)"),
        crate::theme::Appearance::Dark => (ph::MOON, "Appearance: Dark (click to follow macOS)"),
    };
    if icon_button(ui, &p, icon, false, tip).clicked() {
        app.prefs.appearance = app.prefs.appearance.next();
    }
}

// ------------------------------------------------------------------ inspector

fn inspector(app: &mut HinderApp, ctx: &egui::Context, acts: &mut Vec<Act>) {
    let p = app.p;
    egui::SidePanel::right("inspector")
        .exact_width(300.0)
        .resizable(false)
        .frame(Frame::new().fill(p.sidebar).inner_margin(Margin::same(18)).stroke(p.hairline()))
        .show(ctx, |ui| {
            let Some(path) = app.selected.clone() else {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(ph::CURSOR_CLICK).font(f(34.0)).color(p.faint));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Select a file to see its details").color(p.muted));
                });
                return;
            };
            let Some(file) = app.find(&path).cloned() else {
                app.selected = None;
                return;
            };
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let (box_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 200.0), Sense::hover());
                ui.painter().rect_filled(box_rect, CornerRadius::same(12), p.thumb_bg);
                match app.thumbs.get(&file.path) {
                    Some(t) => {
                        let r = image_contain(ui.painter(), t, box_rect.shrink(14.0), CornerRadius::same(6));
                        ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, p.border), StrokeKind::Outside);
                    }
                    None => {
                        ui.painter().text(box_rect.center(), Align2::CENTER_CENTER, file.kind.icon(), f(48.0), p.hue(Palette::kind_slot(file.kind)));
                    }
                }
                ui.add_space(14.0);
                ui.label(RichText::new(&file.name).font(fm(16.0)).color(p.text));
                ui.label(RichText::new(format!("{} · {}", file.type_label(), fmt_size(file.size))).color(p.muted));
                ui.add_space(12.0);

                let row = |ui: &mut Ui, k: &str, v: String| {
                    ui.horizontal(|ui| {
                        ui.add_sized([96.0, 18.0], egui::Label::new(RichText::new(k).color(p.muted).font(f(12.5))));
                        ui.add(egui::Label::new(RichText::new(v).font(f(12.5)).color(p.text)).wrap());
                    });
                };
                row(ui, "Kind", format!("{} ({})", file.type_label(), file.kind.label()));
                row(ui, "Size", fmt_size(file.size));
                if let Some((w, h)) = file.dims {
                    row(ui, "Dimensions", format!("{w} × {h}"));
                }
                row(ui, "Added", fmt_date(file.added));
                row(ui, "Modified", fmt_date(file.modified));
                if let Some(u) = file.last_used {
                    row(ui, "Last opened", fmt_date(u));
                }
                row(ui, "Where", file.parent_display());
                if let Some(src) = &file.source {
                    row(ui, "Downloaded", format!("{} {}", sources::icon_for(&src.key), src.label));
                    for u in src.urls.iter().take(2) {
                        let short = end_truncate(ui, u, &f(11.5), 260.0);
                        ui.hyperlink_to(RichText::new(short).font(f(11.5)), u);
                    }
                }
                ui.add_space(16.0);
                ui.horizontal_wrapped(|ui| {
                    if outline_button(ui, &p, Some(ph::ARROW_SQUARE_OUT), "Open").clicked() {
                        acts.push(Act::Open(path.clone()));
                    }
                    if outline_button(ui, &p, Some(ph::EYE), "Quick Look").clicked() {
                        acts.push(Act::QuickLook(path.clone()));
                    }
                    if outline_button(ui, &p, Some(ph::FOLDER_OPEN), "Show in Finder").clicked() {
                        acts.push(Act::Reveal(path.clone()));
                    }
                    if outline_button(ui, &p, Some(ph::COPY), "Copy Path").clicked() {
                        acts.push(Act::CopyPath(path.clone()));
                    }
                });
                ui.add_space(8.0);
                if solid_button(ui, p.red, Color32::WHITE, Some(ph::TRASH), "Move to Trash").clicked() {
                    acts.push(Act::Trash(vec![path.clone()]));
                }
            });
        });
}

// ------------------------------------------------------------------ overlays

fn indexing(ui: &mut Ui, p: &Palette, logo_tex: Option<&egui::TextureHandle>, n: usize) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.32);
        let (r, _) = ui.allocate_exact_size(vec2(84.0, 84.0), Sense::hover());
        logo(ui.painter(), logo_tex, r);
        ui.add_space(16.0);
        ui.label(RichText::new("Getting to know your files…").font(fm(20.0)).color(p.text));
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} found so far · everything stays on this Mac", fmt_count(n))).color(p.muted));
        ui.add_space(14.0);
        ui.add(egui::Spinner::new().size(20.0).color(p.muted));
    });
}

fn confirm_dialog(app: &mut HinderApp, ctx: &egui::Context, now: f64) {
    let Some(c) = &app.confirm else { return };
    let p = app.p;
    let (title, body, n) = (c.title.clone(), c.body.clone(), c.paths.len());
    let mut decision: Option<bool> = None;
    let resp = egui::Modal::new(Id::new("confirm-trash"))
        .backdrop_color(Color32::from_black_alpha(90))
        .frame(Frame::new().fill(p.surface).corner_radius(CornerRadius::same(16)).inner_margin(Margin::same(22)).stroke(p.hairline()))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(12), crate::theme::mix(p.surface, p.red, 0.14));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, ph::TRASH, f(20.0), p.red);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&title).font(fm(15.0)).color(p.text));
                    ui.add(egui::Label::new(RichText::new(&body).color(p.muted).font(f(13.0))).wrap());
                });
            });
            ui.add_space(18.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if n == 1 { "Move to Trash".to_string() } else { format!("Move {} to Trash", fmt_count(n)) };
                if solid_button(ui, p.red, Color32::WHITE, None, &label).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    decision = Some(true);
                }
                if outline_button(ui, &p, None, "Cancel").clicked() {
                    decision = Some(false);
                }
            });
        });
    if resp.should_close() && decision.is_none() {
        decision = Some(false);
    }
    match decision {
        Some(true) => {
            let paths = app.confirm.take().map(|c| c.paths).unwrap_or_default();
            app.do_trash(paths, now);
        }
        Some(false) => app.confirm = None,
        None => {}
    }
}

fn toast(app: &mut HinderApp, ctx: &egui::Context) {
    let Some((msg, _)) = app.toast.clone() else { return };
    let p = app.p;
    egui::Area::new(Id::new("toast")).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -24.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        let (fill, fg) = if p.dark { (Color32::from_gray(240), Color32::from_gray(20)) } else { (Color32::from_gray(28), Color32::WHITE) };
        Frame::new().fill(fill).corner_radius(CornerRadius::same(12)).inner_margin(Margin::symmetric(16, 10)).shadow(ctx.style().visuals.popup_shadow).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(ph::CHECK_CIRCLE).color(p.green).font(f(16.0)));
                ui.label(RichText::new(&msg).color(fg));
                ui.add_space(8.0);
                if ui.add(egui::Label::new(RichText::new("Open Trash").color(fg).underline()).sense(Sense::click())).clicked() {
                    actions::open_trash();
                }
            });
        });
    });
}
