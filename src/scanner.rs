//! Background file indexer.
//!
//! Walks the user's main folders plus `/Applications`, collecting the metadata
//! Hinder needs. Nothing leaves the machine and nothing is written to disk.

use std::os::macos::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use walkdir::{DirEntry, WalkDir};

use crate::model::{FileEntry, Kind, Library, home_dir, now_secs};
use crate::sources;

/// `SF_DATALESS`: the file's contents live only in iCloud. Reading it would download it.
const SF_DATALESS: u32 = 0x4000_0000;

pub enum ScanMsg {
    Progress(usize),
    Done(Library),
}

/// The folders Hinder looks at. Kept deliberately small and predictable.
pub fn roots() -> Vec<PathBuf> {
    let home = home_dir();
    ["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Music"]
        .iter()
        .map(|d| home.join(d))
        .filter(|p| p.is_dir())
        .collect()
}

pub fn spawn_scan(tx: Sender<ScanMsg>, ctx: eframe::egui::Context) {
    std::thread::Builder::new()
        .name("hinder-scan".into())
        .spawn(move || {
            let lib = scan(|n| {
                let _ = tx.send(ScanMsg::Progress(n));
                ctx.request_repaint();
            });
            let _ = tx.send(ScanMsg::Done(lib));
            ctx.request_repaint();
        })
        .expect("spawn scanner");
}

fn is_skipped_dir(e: &DirEntry) -> bool {
    let name = e.file_name().to_string_lossy();
    if e.depth() > 0 && name.starts_with('.') {
        return true;
    }
    if !e.file_type().is_dir() {
        return false;
    }
    matches!(
        name.as_ref(),
        "node_modules" | "Library" | "target" | "__pycache__" | ".git" | "Pods" | "DerivedData" | "venv" | ".venv"
    ) || [".photoslibrary", ".musiclibrary", ".imovielibrary", ".tvlibrary", ".fcpbundle", ".xcodeproj", ".xcworkspace", ".lrdata", ".photolibrary", ".bundle", ".framework", ".plugin", ".kext"]
        .iter()
        .any(|s| name.ends_with(s))
}

pub fn scan(mut progress: impl FnMut(usize)) -> Library {
    let home = home_dir();
    let downloads = home.join("Downloads");
    let mut paths: Vec<(PathBuf, bool)> = Vec::new(); // (path, is_app_bundle)

    for root in roots() {
        let walker = WalkDir::new(&root).follow_links(false).into_iter().filter_entry(|e| !is_skipped_dir(e));
        let mut it = walker;
        while let Some(next) = it.next() {
            let Ok(e) = next else { continue }; // unreadable folder: skip it, keep going
            let is_dir = e.file_type().is_dir();
            let is_app = is_dir && e.path().extension().is_some_and(|x| x == "app");
            if is_app {
                paths.push((e.into_path(), true));
                it.skip_current_dir();
                continue;
            }
            if e.file_type().is_file() {
                paths.push((e.into_path(), false));
                if paths.len() % 2000 == 0 {
                    progress(paths.len());
                }
            }
        }
    }

    // Apps: top level of /Applications (and ~/Applications), plus one level of sub folders like Utilities.
    for apps_root in [PathBuf::from("/Applications"), home.join("Applications")] {
        for e in WalkDir::new(&apps_root).max_depth(2).into_iter().filter_entry(|e| !is_skipped_dir(e)).flatten() {
            if e.file_type().is_dir() && e.path().extension().is_some_and(|x| x == "app") && !e.path().parent().is_some_and(|p| p.extension().is_some_and(|x| x == "app")) {
                paths.push((e.into_path(), true));
            }
        }
    }
    paths.sort();
    paths.dedup();
    progress(paths.len());

    let files: Vec<FileEntry> = paths
        .par_iter()
        .filter_map(|(p, is_app)| build_entry(p, *is_app, &downloads))
        .collect();

    Library { files, home, scanned_at: now_secs() }
}

fn secs(t: std::io::Result<std::time::SystemTime>) -> i64 {
    t.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn build_entry(path: &Path, is_app: bool, downloads: &Path) -> Option<FileEntry> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if meta.st_flags() & SF_DATALESS != 0 {
        return None; // iCloud-only: never touch it.
    }
    let name = path.file_name()?.to_string_lossy().to_string();
    if name.ends_with(".icloud") || name == "Icon\r" {
        return None;
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let kind = if is_app { Kind::App } else { Kind::from_ext(&ext) };
    let size = if is_app { dir_size(path) } else { meta.len() };
    let modified = secs(meta.modified());
    let added = secs(meta.created()).max(0);

    let is_screenshot = kind == Kind::Image && looks_like_screenshot(path, &name);
    let in_downloads = path.starts_with(downloads);
    let source = if is_app { None } else { sources::read_source(path) };
    let dims = if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
        image::image_dimensions(path).ok()
    } else {
        None
    };

    Some(FileEntry {
        path: path.to_path_buf(),
        name: if is_app { name.trim_end_matches(".app").to_string() } else { name },
        ext,
        size,
        modified,
        added: if added == 0 { modified } else { added },
        last_used: last_used_date(path),
        kind,
        is_screenshot,
        in_downloads,
        source,
        dims,
    })
}

fn looks_like_screenshot(path: &Path, name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.starts_with("screenshot") || n.starts_with("screen shot") || n.starts_with("cleanshot") || n.starts_with("scr-") || n.starts_with("simulator screen") {
        return true;
    }
    // macOS tags its own screen captures.
    xattr::get(path, "com.apple.metadata:kMDItemIsScreenCapture")
        .ok()
        .flatten()
        .is_some()
}

/// `com.apple.lastuseddate#PS` holds a `struct timespec` (16 bytes) written by
/// LaunchServices every time a file or app is opened.
fn last_used_date(path: &Path) -> Option<i64> {
    let raw = xattr::get(path, "com.apple.lastuseddate#PS").ok()??;
    let b: [u8; 8] = raw.get(0..8)?.try_into().ok()?;
    let v = i64::from_le_bytes(b);
    (v > 0).then_some(v)
}

fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| m.st_blocks() * 512)
        })
        .sum()
}
