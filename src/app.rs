//! Application state and the frame loop.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Key, Modifiers};

use crate::cleanup::{self, Card, CardKind};
use crate::dupes::{self, DupGroup};
use crate::model::{DAY, FileEntry, Kind, Library, now_secs};
use crate::query::{self, Query};
use crate::scanner::{self, ScanMsg};
use crate::theme::{self, Look, Palette};
use crate::thumbs::Thumbs;
use crate::{actions, ui};

pub const LARGE_FILE: u64 = 500_000_000;

#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Home,
    Recents,
    Kind(Kind),
    /// A page (a Library shelf, Large Files, Screenshots, a download source…)
    /// narrowed to one folder, browsed folder by folder.
    Inside(Box<View>, PathBuf),
    LargeFiles,
    Screenshots,
    Duplicates,
    CleanUp,
    Review(CardKind),
    Sources,
    Source(String),
    Folder(PathBuf),
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Default)]
pub enum Sort {
    Name,
    #[default]
    Modified,
    Added,
    Size,
    Kind,
}

impl Sort {
    pub const ALL: [Sort; 5] = [Sort::Modified, Sort::Added, Sort::Name, Sort::Size, Sort::Kind];
    pub fn label(self) -> &'static str {
        match self {
            Sort::Name => "Name",
            Sort::Modified => "Date Modified",
            Sort::Added => "Date Added",
            Sort::Size => "Size",
            Sort::Kind => "Kind",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub look: Look,
    pub grid: bool,
    pub zoom: f32,
    pub pinned: Vec<PathBuf>,
    pub inspector: bool,
    pub sidebar: bool,
    pub sort: Sort,
    /// Library shelves browse folder by folder (true) or list every file (false).
    pub kind_folders: bool,
    pub appearance: theme::Appearance,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { look: Look::Vivid, grid: true, zoom: 1.0, pinned: Vec::new(), inspector: false, sidebar: true, sort: Sort::Modified, kind_folders: true, appearance: theme::Appearance::System }
    }
}

/// What a cached folder summary was computed for: (parent, page, scan generation, sort).
type FolderKey = (Option<PathBuf>, Option<View>, u64, Sort);

/// A sub folder shown at the top of a Location view.
#[derive(Clone, Debug)]
pub struct FolderSummary {
    pub path: PathBuf,
    pub name: String,
    pub count: usize,
    pub bytes: u64,
    /// Which main folder it lives in, shown when folders from several are mixed.
    pub within: Option<String>,
}

pub struct SourceSummary {
    pub key: String,
    pub label: String,
    pub count: usize,
    pub bytes: u64,
    pub samples: Vec<PathBuf>,
}

pub struct Confirm {
    pub title: String,
    pub body: String,
    pub paths: Vec<PathBuf>,
}

pub struct HinderApp {
    pub prefs: Prefs,
    pub lib: Library,
    pub generation: u64,
    folder_cache: Option<(FolderKey, Vec<FolderSummary>)>,
    pub scanning: bool,
    pub scan_progress: usize,
    scan_rx: Receiver<ScanMsg>,
    dup_rx: Option<Receiver<Vec<DupGroup>>>,
    pub dups: Vec<DupGroup>,
    pub dups_ready: bool,
    pub cards: Vec<Card>,
    pub sources: Vec<SourceSummary>,
    pub stats: HashMap<Kind, (usize, u64)>,
    /// Newest three files per shelf, for the fanned prints on Home.
    pub shelf_samples: HashMap<Kind, Vec<PathBuf>>,
    /// Indices of the newest files (not apps), for the Recent shelf.
    pub recent: Vec<usize>,
    path_index: HashMap<PathBuf, usize>,
    cleanup_total: u64,
    large_count: usize,
    screenshot_count: usize,
    /// First/last visible row of the grid last frame (for keyboard scrolling).
    pub last_rows: (usize, usize),
    pub view: View,
    pub history: Vec<View>,
    pub search_text: String,
    parsed_for: String,
    pub filters: Query,
    pub content_hits: Option<HashSet<PathBuf>>,
    content_for: String,
    content_rx: Option<Receiver<(String, HashSet<PathBuf>)>>,
    content_due: Option<(String, f64)>,
    list_cache: Option<(u64, Vec<usize>)>,
    pub selected: Option<PathBuf>,
    pub review_sel: HashSet<PathBuf>,
    pub review_for: Option<CardKind>,
    pub confirm: Option<Confirm>,
    pub toast: Option<(String, f64)>,
    pub thumbs: Thumbs,
    pub logo: Option<egui::TextureHandle>,
    pub p: Palette,
    pub focus_search: bool,
    pub grid_cols: usize,
    /// Arrow-key movement requested this frame (dx, dy), applied by the grid.
    pub nav_delta: Option<(i64, i64)>,
    rescan_requested: bool,
    back_requested: bool,
    pub trash_requested: Option<Vec<PathBuf>>,
}

impl HinderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let prefs: Prefs = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        let (tx, rx) = mpsc::channel();
        scanner::spawn_scan(tx, cc.egui_ctx.clone());
        let dark = cc.egui_ctx.style().visuals.dark_mode;
        let p = Palette::new(prefs.look, dark);
        theme::apply_visuals(&cc.egui_ctx, &p);
        Self {
            prefs,
            lib: Library::default(),
            generation: 0,
            folder_cache: None,
            scanning: true,
            scan_progress: 0,
            scan_rx: rx,
            dup_rx: None,
            dups: Vec::new(),
            dups_ready: false,
            cards: Vec::new(),
            sources: Vec::new(),
            stats: HashMap::new(),
            shelf_samples: HashMap::new(),
            recent: Vec::new(),
            path_index: HashMap::new(),
            cleanup_total: 0,
            large_count: 0,
            screenshot_count: 0,
            last_rows: (0, 0),
            view: View::Home,
            history: Vec::new(),
            search_text: String::new(),
            parsed_for: String::new(),
            filters: Query::default(),
            content_hits: None,
            content_for: String::new(),
            content_rx: None,
            content_due: None,
            list_cache: None,
            selected: None,
            review_sel: HashSet::new(),
            review_for: None,
            confirm: None,
            toast: None,
            thumbs: Thumbs::new(&cc.egui_ctx),
            logo: crate::brand::logo_texture(&cc.egui_ctx, 84),
            p,
            focus_search: false,
            grid_cols: 4,
            nav_delta: None,
            rescan_requested: false,
            back_requested: false,
            trash_requested: None,
        }
    }

    // ---------------------------------------------------------------- navigation

    pub fn go(&mut self, view: View) {
        if self.view == view {
            return;
        }
        if view != View::Search {
            self.search_text.clear();
            self.parsed_for.clear();
            self.filters = Query::default();
            self.content_hits = None;
        }
        let prev = std::mem::replace(&mut self.view, view);
        if prev != View::Search {
            self.history.push(prev);
        }
        if self.history.len() > 50 {
            self.history.remove(0);
        }
        self.list_cache = None;
    }

    pub fn back(&mut self) {
        if let Some(v) = self.history.pop() {
            if self.view == View::Search {
                self.search_text.clear();
                self.parsed_for.clear();
                self.filters = Query::default();
            }
            self.view = v;
            self.list_cache = None;
        }
    }

    pub fn rescan(&mut self, ctx: &egui::Context) {
        if self.scanning {
            return;
        }
        let (tx, rx) = mpsc::channel();
        scanner::spawn_scan(tx, ctx.clone());
        self.scan_rx = rx;
        self.scanning = true;
        self.scan_progress = 0;
    }

    // ---------------------------------------------------------------- derived data

    fn recompute(&mut self) {
        self.generation += 1;
        self.list_cache = None;
        self.stats.clear();
        for f in &self.lib.files {
            let e = self.stats.entry(f.kind).or_default();
            e.0 += 1;
            e.1 += f.size;
        }
        let mut map: HashMap<String, SourceSummary> = HashMap::new();
        let mut recent_first: Vec<&FileEntry> = self.lib.files.iter().filter(|f| f.source.is_some()).collect();
        recent_first.sort_by_key(|f| std::cmp::Reverse(f.date()));
        for f in recent_first {
            let s = f.source.as_ref().unwrap();
            let e = map.entry(s.key.clone()).or_insert_with(|| SourceSummary { key: s.key.clone(), label: s.label.clone(), count: 0, bytes: 0, samples: Vec::new() });
            e.count += 1;
            e.bytes += f.size;
            if e.samples.len() < 3 && matches!(f.kind, Kind::Image | Kind::Video | Kind::Document) {
                e.samples.push(f.path.clone());
            }
        }
        self.sources = map.into_values().collect();
        self.sources.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
        self.path_index = self.lib.files.iter().enumerate().map(|(i, f)| (f.path.clone(), i)).collect();
        self.large_count = self.lib.files.iter().filter(|f| f.size >= LARGE_FILE && f.kind != Kind::App).count();
        self.screenshot_count = self.lib.files.iter().filter(|f| f.is_screenshot).count();
        self.refresh_cards();

        let mut by_date: Vec<usize> = (0..self.lib.files.len()).collect();
        by_date.sort_by_key(|&i| std::cmp::Reverse(self.lib.files[i].date()));
        self.shelf_samples.clear();
        for &i in &by_date {
            let f = &self.lib.files[i];
            let v = self.shelf_samples.entry(f.kind).or_default();
            if v.len() < 3 {
                v.push(f.path.clone());
            }
        }
        self.recent = by_date.into_iter().filter(|&i| self.lib.files[i].kind != Kind::App).take(12).collect();
    }

    /// Recompute Clean Up cards (after duplicates change or a keeper is picked).
    pub fn refresh_cards(&mut self) {
        self.cards = cleanup::compute(&self.lib, &self.dups);
        self.cleanup_total = cleanup::total(&self.cards);
    }

    fn start_dupes(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let files = self.lib.files.clone();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("hinder-dupes".into())
            .spawn(move || {
                let _ = tx.send(dupes::find(&files));
                ctx.request_repaint();
            })
            .ok();
        self.dup_rx = Some(rx);
        self.dups_ready = false;
    }

    pub fn source_label(&self, key: &str) -> String {
        self.sources.iter().find(|s| s.key == key).map(|s| s.label.clone()).unwrap_or_else(|| key.to_string())
    }

    pub fn find(&self, path: &std::path::Path) -> Option<&FileEntry> {
        self.path_index.get(path).and_then(|&i| self.lib.files.get(i))
    }

    pub fn card(&self, kind: CardKind) -> Option<&Card> {
        self.cards.iter().find(|c| c.kind == kind)
    }

    pub fn cleanup_total(&self) -> u64 {
        self.cleanup_total
    }

    pub fn large_count(&self) -> usize {
        self.large_count
    }

    pub fn screenshot_count(&self) -> usize {
        self.screenshot_count
    }

    /// The page the current view belongs to: `Inside(Kind(Documents), dir)` → `Kind(Documents)`.
    pub fn base_view(&self) -> &View {
        match &self.view {
            View::Inside(b, _) => b,
            v => v,
        }
    }

    /// Pages that can be browsed folder by folder. Recents stays a date-ordered
    /// list and Apps a single A–Z grid, like Finder's Applications.
    pub fn can_browse(view: &View) -> bool {
        match view {
            View::Kind(k) => *k != Kind::App,
            View::LargeFiles | View::Screenshots | View::Source(_) => true,
            _ => false,
        }
    }

    /// The sort actually used: Apps default to A–Z and Large Files to biggest first.
    pub fn effective_sort(&self) -> Sort {
        match (self.base_view(), self.prefs.sort) {
            (View::LargeFiles, Sort::Modified) => Sort::Size,
            (View::Kind(Kind::App), Sort::Modified) => Sort::Name,
            (_, s) => s,
        }
    }

    /// The folders a page's first screen groups by: the main folders plus Applications.
    fn top_parents() -> Vec<PathBuf> {
        let mut v = scanner::roots();
        for p in [PathBuf::from("/Applications"), crate::model::home_dir().join("Applications")] {
            if p.is_dir() {
                v.push(p);
            }
        }
        v
    }

    /// Whether a file belongs on a page, ignoring which folder is open.
    fn in_page(view: &View, f: &FileEntry, now: i64) -> bool {
        match view {
            View::Recents => f.kind != Kind::App && now - f.date() <= 30 * DAY,
            View::Kind(k) => f.kind == *k,
            View::LargeFiles => f.size >= LARGE_FILE && f.kind != Kind::App,
            View::Screenshots => f.is_screenshot,
            View::Source(key) => f.source.as_ref().is_some_and(|s| &s.key == key),
            _ => true,
        }
    }

    /// Whether the current view browses folder by folder (sub folders first,
    /// then only the files directly inside) rather than listing everything.
    pub fn browsing_folders(&self) -> bool {
        if !self.filters.is_empty() {
            return false; // a filter searches the whole tree
        }
        match &self.view {
            View::Folder(_) => true,
            _ => self.prefs.kind_folders && Self::can_browse(self.base_view()),
        }
    }

    /// Folders to show at the top of the current view, with their file count and size.
    pub fn view_folders(&mut self) -> Vec<FolderSummary> {
        if !self.browsing_folders() {
            return Vec::new();
        }
        match self.view.clone() {
            View::Folder(dir) => self.subfolders(Some(&dir), None),
            View::Inside(page, dir) => self.subfolders(Some(&dir), Some(*page)),
            page => self.subfolders(None, Some(page)),
        }
    }

    /// Sub folders of `dir` that contain indexed files, optionally only counting
    /// files that belong on `page`. With `dir = None`, the sub folders of every
    /// main folder (Desktop, Documents, Downloads…, Applications) are combined, so
    /// the first screen shows real folders instead of one tile per main folder.
    /// Derived from the index (no disk access) and cached.
    pub fn subfolders(&mut self, dir: Option<&std::path::Path>, page: Option<View>) -> Vec<FolderSummary> {
        let key: FolderKey = (dir.map(|d| d.to_path_buf()), page.clone(), self.generation, self.prefs.sort);
        if let Some((k, v)) = &self.folder_cache {
            if *k == key {
                return v.clone();
            }
        }
        let parents: Vec<PathBuf> = match dir {
            Some(d) => vec![d.to_path_buf()],
            None => Self::top_parents(),
        };
        let now = now_secs();
        let mut map: std::collections::BTreeMap<PathBuf, (usize, u64)> = std::collections::BTreeMap::new();
        for f in self.lib.files.iter().filter(|f| page.as_ref().is_none_or(|pg| Self::in_page(pg, f, now))) {
            let Some((parent, rest)) = parents.iter().find_map(|p| f.path.strip_prefix(p).ok().map(|r| (p, r))) else { continue };
            let mut comps = rest.components();
            let (Some(first), Some(_)) = (comps.next(), comps.next()) else { continue };
            let e = map.entry(parent.join(first)).or_default();
            e.0 += 1;
            e.1 += f.size;
        }
        let mut v: Vec<FolderSummary> = map
            .into_iter()
            .map(|(path, (count, bytes))| FolderSummary {
                name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                within: dir.is_none().then(|| path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned())).flatten(),
                path,
                count,
                bytes,
            })
            .collect();
        if v.windows(2).all(|w| w[0].within == w[1].within) {
            v.iter_mut().for_each(|f| f.within = None); // all from one main folder: no need to say so
        }
        match self.prefs.sort {
            Sort::Size => v.sort_by_key(|s| std::cmp::Reverse(s.bytes)),
            _ => v.sort_by_cached_key(|s| s.name.to_lowercase()),
        }
        self.folder_cache = Some((key, v.clone()));
        v
    }

    /// The list of files for the current list-style view, filtered and sorted.
    pub fn visible(&mut self) -> Vec<usize> {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}|{:?}|{:?}|{}", self.view, self.filters, self.prefs.sort, self.prefs.kind_folders).hash(&mut h);
        self.generation.hash(&mut h);
        self.content_hits.as_ref().map(|c| c.len()).hash(&mut h);
        let key = h.finish();
        if let Some((k, v)) = &self.list_cache {
            if *k == key {
                return v.clone();
            }
        }
        let now = now_secs();
        let browsing = self.browsing_folders();
        let tops = Self::top_parents();
        let base = |f: &FileEntry| -> bool {
            let parent = f.path.parent();
            match &self.view {
                // Like Finder: only what's directly inside. With a filter set,
                // search the whole folder tree instead.
                View::Folder(p) if browsing => parent == Some(p.as_path()),
                View::Folder(p) => f.path.starts_with(p),
                View::Inside(page, p) if browsing => Self::in_page(page, f, now) && parent == Some(p.as_path()),
                View::Inside(page, p) => Self::in_page(page, f, now) && f.path.starts_with(p),
                // First screen of a page: files lying loose in Desktop, Documents, Downloads…
                page if browsing => Self::in_page(page, f, now) && parent.is_some_and(|d| tops.iter().any(|t| t == d)),
                page => Self::in_page(page, f, now),
            }
        };
        let hits = self.content_hits.as_ref();
        let mut idx: Vec<usize> = self
            .lib
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| base(f) && self.filters.matches(f, hits))
            .map(|(i, _)| i)
            .collect();
        let files = &self.lib.files;
        let sort = self.effective_sort();
        match sort {
            Sort::Name => idx.sort_by_cached_key(|&i| files[i].name.to_lowercase()),
            Sort::Modified => idx.sort_by_key(|&i| std::cmp::Reverse(files[i].modified)),
            Sort::Added => idx.sort_by_key(|&i| std::cmp::Reverse(files[i].added)),
            Sort::Size => idx.sort_by_key(|&i| std::cmp::Reverse(files[i].size)),
            Sort::Kind => idx.sort_by_key(|&i| (files[i].kind, files[i].ext.clone(), files[i].name.to_lowercase())),
        }
        self.list_cache = Some((key, idx.clone()));
        idx
    }

    // ---------------------------------------------------------------- search

    pub fn on_search_changed(&mut self, now: f64) {
        if self.search_text == self.parsed_for {
            return;
        }
        self.parsed_for = self.search_text.clone();
        if self.search_text.trim().is_empty() {
            if self.view == View::Search {
                self.back();
            }
            return;
        }
        let known: Vec<(String, String)> = self.sources.iter().map(|s| (s.key.clone(), s.label.clone())).collect();
        self.filters = query::parse(&self.search_text, &known);
        if self.view != View::Search {
            let prev = std::mem::replace(&mut self.view, View::Search);
            self.history.push(prev);
        }
        self.list_cache = None;
        // Debounced Spotlight content search for the free-text part.
        let phrase = self.filters.text_phrase();
        if phrase.is_empty() {
            self.content_hits = None;
            self.content_for.clear();
        } else if phrase != self.content_for {
            self.content_due = Some((phrase, now + 0.35));
        }
    }

    fn poll_content_search(&mut self, ctx: &egui::Context, now: f64) {
        if let Some((phrase, due)) = self.content_due.clone() {
            if now >= due {
                self.content_due = None;
                let (tx, rx) = mpsc::channel();
                let roots = scanner::roots();
                let ctx2 = ctx.clone();
                std::thread::spawn(move || {
                    let hits: HashSet<PathBuf> = actions::spotlight_search(&phrase, &roots).into_iter().collect();
                    let _ = tx.send((phrase, hits));
                    ctx2.request_repaint();
                });
                self.content_rx = Some(rx);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(due - now));
            }
        }
        if let Some(rx) = &self.content_rx {
            if let Ok((phrase, hits)) = rx.try_recv() {
                if phrase == self.filters.text_phrase() {
                    self.content_for = phrase;
                    self.content_hits = Some(hits);
                    self.list_cache = None;
                }
                self.content_rx = None;
            }
        }
    }

    // ---------------------------------------------------------------- trash

    pub fn ask_trash(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let bytes: u64 = paths.iter().filter_map(|p| self.find(p)).map(|f| f.size).sum();
        let title = if paths.len() == 1 {
            format!("Move “{}” to the Trash?", paths[0].file_name().unwrap_or_default().to_string_lossy())
        } else {
            format!("Move {} items to the Trash?", crate::model::fmt_count(paths.len()))
        };
        let body = format!("This frees about {}. You can put them back from the Trash any time.", crate::model::fmt_size(bytes));
        self.confirm = Some(Confirm { title, body, paths });
    }

    pub fn do_trash(&mut self, paths: Vec<PathBuf>, now: f64) {
        let bytes: u64 = paths.iter().filter_map(|p| self.find(p)).map(|f| f.size).sum();
        let (ok, err) = actions::move_to_trash(&paths);
        let gone: HashSet<PathBuf> = ok.iter().cloned().collect();
        self.lib.remove_paths(&gone);
        for g in &mut self.dups {
            let keep_path = g.paths[g.keep].clone();
            g.paths.retain(|p| !gone.contains(p));
            g.keep = g.paths.iter().position(|p| *p == keep_path).unwrap_or(0);
        }
        self.dups.retain(|g| g.paths.len() > 1);
        for p in &ok {
            self.thumbs.forget(p);
            self.review_sel.remove(p);
        }
        if self.selected.as_ref().is_some_and(|s| gone.contains(s)) {
            self.selected = None;
        }
        self.recompute();
        let msg = match err {
            Some(e) if ok.is_empty() => e,
            Some(e) => format!("Moved {} to the Trash. {e}", crate::model::plural(ok.len(), "item", "items")),
            None => format!("Moved {} to the Trash · {} freed", crate::model::plural(ok.len(), "item", "items"), crate::model::fmt_size(bytes)),
        };
        self.toast = Some((msg, now + 5.0));
    }

    // ---------------------------------------------------------------- input

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.wants_keyboard_input();
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let ctrl_cmd = Modifiers::COMMAND | Modifiers::CTRL;
        ctx.input_mut(|i| {
            if i.consume_key(cmd, Key::F) {
                self.focus_search = true;
            }
            if i.consume_key(cmd_shift, Key::D) {
                self.go(View::Duplicates);
            }
            if i.consume_key(cmd_shift, Key::K) {
                self.go(View::CleanUp);
            }
            if i.consume_key(cmd_shift, Key::J) {
                self.go(View::Sources);
            }
            if i.consume_key(cmd_shift, Key::H) {
                self.go(View::Home);
            }
            if i.consume_key(ctrl_cmd, Key::T) {
                self.prefs.look = self.prefs.look.next();
            }
            if i.consume_key(cmd, Key::Num1) {
                self.prefs.grid = true;
            }
            if i.consume_key(cmd, Key::Num2) {
                self.prefs.grid = false;
            }
            if i.consume_key(cmd, Key::Plus) || i.consume_key(cmd, Key::Equals) {
                self.prefs.zoom = (self.prefs.zoom + 0.15).min(2.0);
            }
            if i.consume_key(cmd, Key::Minus) {
                self.prefs.zoom = (self.prefs.zoom - 0.15).max(0.6);
            }
            if i.consume_key(cmd, Key::I) {
                self.prefs.inspector = !self.prefs.inspector;
            }
            if i.consume_key(cmd, Key::R) {
                self.rescan_requested = true;
            }
            if i.consume_key(cmd, Key::OpenBracket) {
                self.back_requested = true;
            }
            if !typing {
                if i.consume_key(Modifiers::NONE, Key::Space) {
                    if let Some(s) = &self.selected {
                        actions::quick_look(std::slice::from_ref(s));
                    }
                }
                if i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(cmd, Key::O) || i.consume_key(cmd, Key::ArrowDown) {
                    if let Some(s) = &self.selected {
                        actions::open(s);
                    }
                }
                if i.consume_key(cmd, Key::Backspace) || i.consume_key(cmd, Key::Delete) {
                    if let Some(s) = self.selected.clone() {
                        self.trash_requested = Some(vec![s]);
                    }
                }
                for (key, dx, dy) in [(Key::ArrowLeft, -1i64, 0i64), (Key::ArrowRight, 1, 0), (Key::ArrowUp, 0, -1), (Key::ArrowDown, 0, 1)] {
                    if i.consume_key(Modifiers::NONE, key) {
                        self.nav_delta = Some((dx, dy));
                    }
                }
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                if !self.search_text.is_empty() {
                    self.search_text.clear();
                } else if self.confirm.is_some() {
                    self.confirm = None;
                } else {
                    self.selected = None;
                }
            }
        });
    }
}

impl eframe::App for HinderApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.prefs);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);

        // Background results
        while let Ok(msg) = self.scan_rx.try_recv() {
            match msg {
                ScanMsg::Progress(n) => self.scan_progress = n,
                ScanMsg::Done(lib) => {
                    self.lib = lib;
                    self.scanning = false;
                    self.recompute();
                    self.start_dupes(ctx);
                }
            }
        }
        if let Some(rx) = &self.dup_rx {
            if let Ok(groups) = rx.try_recv() {
                // Drop paths trashed while hashing was running.
                let alive: HashSet<&PathBuf> = self.lib.files.iter().map(|f| &f.path).collect();
                self.dups = groups
                    .into_iter()
                    .map(|mut g| {
                        let keep = g.paths[g.keep].clone();
                        g.paths.retain(|p| alive.contains(p));
                        g.keep = g.paths.iter().position(|p| *p == keep).unwrap_or(0);
                        g
                    })
                    .filter(|g| g.paths.len() > 1)
                    .collect();
                self.dups_ready = true;
                self.dup_rx = None;
                self.recompute();
            }
        }
        self.poll_content_search(ctx, now);

        // Light/dark follows macOS unless forced in prefs; look follows prefs.
        let dark = match self.prefs.appearance {
            theme::Appearance::Light => false,
            theme::Appearance::Dark => true,
            theme::Appearance::System => match ctx.system_theme() {
                Some(egui::Theme::Dark) => true,
                Some(egui::Theme::Light) => false,
                None => ctx.style().visuals.dark_mode,
            },
        };
        if self.p.dark != dark || self.p.look != self.prefs.look {
            self.p = Palette::new(self.prefs.look, dark);
            theme::apply_visuals(ctx, &self.p);
        }

        self.thumbs.begin_frame(ctx);
        self.shortcuts(ctx);
        if std::mem::take(&mut self.rescan_requested) {
            self.rescan(ctx);
        }
        if std::mem::take(&mut self.back_requested) {
            self.back();
        }
        if let Some(paths) = self.trash_requested.take() {
            self.ask_trash(paths);
        }

        ui::draw(self, ctx, now);

        if let Some((_, until)) = &self.toast {
            if now > *until {
                self.toast = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        }
    }
}
