//! The native macOS menu bar: Hinder, File, View, Go, Window and Help.
//!
//! Menu items with a shortcut take that key before the window sees it, so every
//! item is routed to a [`Cmd`] that the app runs exactly like the matching
//! keyboard shortcut. Shortcuts that would clash with typing in the search box
//! (Space, Return, arrows, ⌘⌫) are listed in the menu without a key equivalent
//! and keep working through the window's own key handling.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use muda::accelerator::{Accelerator, Code, Modifiers as M};
use muda::{AboutMetadata, CheckMenuItem, Icon, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use crate::model::Kind;
use crate::theme::{Appearance, Look};

pub const REPO_URL: &str = "https://github.com/yaasiinaxmed/Hinder-Mac-App";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    CheckForUpdates,
    // File
    Open,
    QuickLook,
    ShowInFinder,
    CopyPath,
    MoveToTrash,
    OpenTrash,
    Find,
    // View
    Grid,
    List,
    Bigger,
    Smaller,
    Sidebar,
    Inspector,
    Look(Look),
    NextLook,
    Appearance(Appearance),
    Rescan,
    // Go
    Back,
    Home,
    Recents,
    Kind(Kind),
    LargeFiles,
    Screenshots,
    Duplicates,
    CleanUp,
    Sources,
    Folder(&'static str),
    // Help
    Website,
    ReportIssue,
}

pub struct MenuBar {
    _menu: Menu,
    ids: HashMap<MenuId, Cmd>,
    rx: Receiver<MenuEvent>,
    grid: CheckMenuItem,
    list: CheckMenuItem,
    sidebar: CheckMenuItem,
    inspector: CheckMenuItem,
    looks: Vec<(Look, CheckMenuItem)>,
    appearances: Vec<(Appearance, CheckMenuItem)>,
}

fn acc(mods: M, code: Code) -> Option<Accelerator> {
    Some(Accelerator::new(Some(mods), code))
}

const CMD: M = M::SUPER;

impl MenuBar {
    /// Builds the menu bar and installs it. Call once the app is running (from
    /// the eframe creation callback), on the main thread.
    pub fn install(ctx: &egui::Context, icon_rgba: Vec<u8>, icon_px: u32) -> Self {
        let mut ids: HashMap<MenuId, Cmd> = HashMap::new();
        let mut n = 0usize;
        let mut item = |ids: &mut HashMap<MenuId, Cmd>, text: &str, cmd: Cmd, a: Option<Accelerator>| {
            n += 1;
            let id = MenuId::new(format!("hinder.{n}"));
            ids.insert(id.clone(), cmd);
            MenuItem::with_id(id, text, true, a)
        };
        let mut check_n = 0usize;
        let mut check = |ids: &mut HashMap<MenuId, Cmd>, text: &str, cmd: Cmd, a: Option<Accelerator>| {
            check_n += 1;
            let id = MenuId::new(format!("hinder.check.{check_n}"));
            ids.insert(id.clone(), cmd);
            CheckMenuItem::with_id(id, text, true, false, a)
        };
        let sep = PredefinedMenuItem::separator;

        // ---- Hinder
        let about = AboutMetadata {
            name: Some("Hinder".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            comments: Some("A friendlier way to see, search and tidy the files on your Mac.".into()),
            copyright: Some("Open source under the MIT license".into()),
            website: Some(REPO_URL.into()),
            icon: Icon::from_rgba(icon_rgba, icon_px, icon_px).ok(),
            ..Default::default()
        };
        let updates = item(&mut ids, "Check for Updates…", Cmd::CheckForUpdates, None);
        let app_menu = Submenu::with_items(
            "Hinder",
            true,
            &[
                &PredefinedMenuItem::about(Some("About Hinder"), Some(about)),
                &updates,
                &sep(),
                &PredefinedMenuItem::services(None),
                &sep(),
                &PredefinedMenuItem::hide(Some("Hide Hinder")),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &sep(),
                &PredefinedMenuItem::quit(Some("Quit Hinder")),
            ],
        )
        .expect("app menu");

        // ---- File
        let file_items: Vec<Box<dyn IsMenuItem>> = vec![
            Box::new(item(&mut ids, "Open", Cmd::Open, acc(CMD, Code::KeyO))),
            Box::new(item(&mut ids, "Quick Look   (Space)", Cmd::QuickLook, acc(CMD, Code::KeyY))),
            Box::new(item(&mut ids, "Show in Finder", Cmd::ShowInFinder, acc(CMD | M::ALT, Code::KeyR))),
            Box::new(item(&mut ids, "Copy Path", Cmd::CopyPath, acc(CMD | M::ALT, Code::KeyC))),
            Box::new(sep()),
            Box::new(item(&mut ids, "Move to Trash   (⌘⌫)", Cmd::MoveToTrash, None)),
            Box::new(item(&mut ids, "Open Trash", Cmd::OpenTrash, None)),
            Box::new(sep()),
            Box::new(item(&mut ids, "Find…", Cmd::Find, acc(CMD, Code::KeyF))),
            Box::new(sep()),
            Box::new(PredefinedMenuItem::close_window(Some("Close Window"))),
        ];
        let file = Submenu::with_items("File", true, &refs(&file_items)).expect("file menu");

        // ---- View
        let grid = check(&mut ids, "as Grid", Cmd::Grid, acc(CMD, Code::Digit1));
        let list = check(&mut ids, "as List", Cmd::List, acc(CMD, Code::Digit2));
        let sidebar = check(&mut ids, "Sidebar", Cmd::Sidebar, acc(CMD | M::ALT, Code::KeyS));
        let inspector = check(&mut ids, "Inspector", Cmd::Inspector, acc(CMD, Code::KeyI));
        let looks: Vec<(Look, CheckMenuItem)> = [(Look::Vivid, "Vivid (Hinder copper)"), (Look::Normal, "Normal (macOS greys)"), (Look::Mono, "Mono (black and white)")]
            .into_iter()
            .map(|(l, t)| (l, check(&mut ids, t, Cmd::Look(l), None)))
            .collect();
        let next_look = item(&mut ids, "Next Look", Cmd::NextLook, acc(CMD | M::CONTROL, Code::KeyT));
        let appearances: Vec<(Appearance, CheckMenuItem)> = [(Appearance::System, "Follow macOS"), (Appearance::Light, "Light"), (Appearance::Dark, "Dark")]
            .into_iter()
            .map(|(a, t)| (a, check(&mut ids, t, Cmd::Appearance(a), None)))
            .collect();
        let look_menu = {
            let mut v: Vec<&dyn IsMenuItem> = looks.iter().map(|(_, i)| i as &dyn IsMenuItem).collect();
            v.push(&next_look);
            Submenu::with_items("Look", true, &v).expect("look menu")
        };
        let appearance_menu = {
            let v: Vec<&dyn IsMenuItem> = appearances.iter().map(|(_, i)| i as &dyn IsMenuItem).collect();
            Submenu::with_items("Appearance", true, &v).expect("appearance menu")
        };
        let bigger = item(&mut ids, "Bigger Cards", Cmd::Bigger, acc(CMD, Code::Equal));
        let smaller = item(&mut ids, "Smaller Cards", Cmd::Smaller, acc(CMD, Code::Minus));
        let rescan = item(&mut ids, "Refresh", Cmd::Rescan, acc(CMD, Code::KeyR));
        let view = Submenu::with_items(
            "View",
            true,
            &[
                &grid,
                &list,
                &sep(),
                &bigger,
                &smaller,
                &sep(),
                &sidebar,
                &inspector,
                &sep(),
                &look_menu,
                &appearance_menu,
                &sep(),
                &rescan,
                &sep(),
                &PredefinedMenuItem::fullscreen(None),
            ],
        )
        .expect("view menu");

        // ---- Go
        let mut go_items: Vec<Box<dyn IsMenuItem>> = vec![
            Box::new(item(&mut ids, "Back", Cmd::Back, acc(CMD, Code::BracketLeft))),
            Box::new(sep()),
            Box::new(item(&mut ids, "Home", Cmd::Home, acc(CMD | M::SHIFT, Code::KeyH))),
            Box::new(item(&mut ids, "Recents", Cmd::Recents, acc(CMD | M::SHIFT, Code::KeyF))),
            Box::new(sep()),
        ];
        for k in Kind::SHELVES {
            go_items.push(Box::new(item(&mut ids, k.label(), Cmd::Kind(k), None)));
        }
        go_items.extend::<Vec<Box<dyn IsMenuItem>>>(vec![
            Box::new(sep()),
            Box::new(item(&mut ids, "Large Files", Cmd::LargeFiles, None)),
            Box::new(item(&mut ids, "Screenshots", Cmd::Screenshots, None)),
            Box::new(item(&mut ids, "Duplicates", Cmd::Duplicates, acc(CMD | M::SHIFT, Code::KeyD))),
            Box::new(item(&mut ids, "Clean Up", Cmd::CleanUp, acc(CMD | M::SHIFT, Code::KeyK))),
            Box::new(item(&mut ids, "Downloaded From", Cmd::Sources, acc(CMD | M::SHIFT, Code::KeyJ))),
            Box::new(sep()),
            Box::new(item(&mut ids, "Desktop", Cmd::Folder("Desktop"), None)),
            Box::new(item(&mut ids, "Documents", Cmd::Folder("Documents"), None)),
            Box::new(item(&mut ids, "Downloads", Cmd::Folder("Downloads"), acc(CMD | M::ALT, Code::KeyL))),
            Box::new(item(&mut ids, "Pictures", Cmd::Folder("Pictures"), None)),
        ]);
        let go = Submenu::with_items("Go", true, &refs(&go_items)).expect("go menu");

        // ---- Window
        let window = Submenu::with_items(
            "Window",
            true,
            &[
                &PredefinedMenuItem::minimize(None),
                &PredefinedMenuItem::maximize(Some("Zoom")),
                &sep(),
                &PredefinedMenuItem::bring_all_to_front(None),
            ],
        )
        .expect("window menu");

        // ---- Help
        let help = Submenu::with_items(
            "Help",
            true,
            &[&item(&mut ids, "Hinder on GitHub", Cmd::Website, None), &item(&mut ids, "Report an Issue…", Cmd::ReportIssue, None)],
        )
        .expect("help menu");

        let menu = Menu::with_items(&[&app_menu, &file, &view, &go, &window, &help]).expect("menu bar");
        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
            window.set_as_windows_menu_for_nsapp();
            help.set_as_help_menu_for_nsapp();
        }

        // Menu clicks arrive on the main thread; wake egui so they run right away.
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let _ = tx.send(e);
            ctx.request_repaint();
        }));

        Self { _menu: menu, ids, rx, grid, list, sidebar, inspector, looks, appearances }
    }

    /// Commands chosen since the last frame.
    pub fn take(&self) -> Vec<Cmd> {
        self.rx.try_iter().filter_map(|e| self.ids.get(e.id()).copied()).collect()
    }

    /// Keeps the checkmarks in step with the current settings.
    pub fn sync(&self, grid: bool, sidebar: bool, inspector: bool, look: Look, appearance: Appearance) {
        let set = |i: &CheckMenuItem, v: bool| {
            if i.is_checked() != v {
                i.set_checked(v);
            }
        };
        set(&self.grid, grid);
        set(&self.list, !grid);
        set(&self.sidebar, sidebar);
        set(&self.inspector, inspector);
        for (l, i) in &self.looks {
            set(i, *l == look);
        }
        for (a, i) in &self.appearances {
            set(i, *a == appearance);
        }
    }
}

fn refs(items: &[Box<dyn IsMenuItem>]) -> Vec<&dyn IsMenuItem> {
    items.iter().map(|b| b.as_ref()).collect()
}
