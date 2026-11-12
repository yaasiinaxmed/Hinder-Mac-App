//! Things Hinder asks macOS to do. Deleting is never permanent: files go to the Trash.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn open(path: &Path) {
    let _ = Command::new("/usr/bin/open").arg(path).spawn();
}

pub fn reveal_in_finder(path: &Path) {
    let _ = Command::new("/usr/bin/open").arg("-R").arg(path).spawn();
}

/// Real macOS Quick Look panel for one or more files.
pub fn quick_look(paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }
    let _ = Command::new("/usr/bin/qlmanage")
        .arg("-p")
        .args(paths)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Moves files to the Trash (with "Put Back" support, via Finder). Returns the
/// paths that were trashed and an error message for the rest, if any.
pub fn move_to_trash(paths: &[PathBuf]) -> (Vec<PathBuf>, Option<String>) {
    let mut ok = Vec::new();
    let mut failed = Vec::new();
    for p in paths {
        match trash::delete(p) {
            Ok(()) => ok.push(p.clone()),
            Err(e) => failed.push(format!("{}: {e}", p.file_name().unwrap_or_default().to_string_lossy())),
        }
    }
    let err = (!failed.is_empty()).then(|| {
        let more = if failed.len() > 3 { format!(" and {} more", failed.len() - 3) } else { String::new() };
        format!("Couldn't move {}{}", failed.into_iter().take(3).collect::<Vec<_>>().join(", "), more)
    });
    (ok, err)
}

pub fn open_trash() {
    let _ = Command::new("/usr/bin/open").arg(dirs::home_dir().unwrap_or_default().join(".Trash")).spawn();
}

/// Full-text search via Spotlight (`mdfind`). Spotlight also indexes the text
/// macOS recognises inside images, so this finds words in screenshots too.
pub fn spotlight_search(phrase: &str, roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(o) = Command::new("/usr/bin/mdfind").arg("-onlyin").arg(root).arg(phrase).output() else { continue };
        out.extend(String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).map(PathBuf::from));
    }
    out
}
