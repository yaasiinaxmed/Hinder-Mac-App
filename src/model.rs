//! Core data types shared by the scanner, search engine and UI.

use std::path::{Path, PathBuf};

use egui_phosphor::regular as ph;

/// The six "shelves" Hinder sorts every file onto, plus a catch-all.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Image,
    Video,
    Music,
    Document,
    Archive,
    App,
    Other,
}

impl Kind {
    pub const SHELVES: [Kind; 6] = [
        Kind::Image,
        Kind::Video,
        Kind::Music,
        Kind::Document,
        Kind::Archive,
        Kind::App,
    ];

    pub fn from_ext(ext: &str) -> Kind {
        match ext {
            "png" | "jpg" | "jpeg" | "heic" | "heif" | "gif" | "webp" | "tiff" | "tif" | "bmp"
            | "svg" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "avif" | "ico" | "icns"
            | "psd" => Kind::Image,
            "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" | "wmv" | "flv" | "mpg" | "mpeg"
            | "3gp" => Kind::Video,
            "mp3" | "m4a" | "aac" | "wav" | "flac" | "aiff" | "aif" | "ogg" | "opus" | "alac"
            | "wma" | "mid" | "midi" => Kind::Music,
            "pdf" | "doc" | "docx" | "pages" | "txt" | "rtf" | "md" | "markdown" | "odt" | "xls"
            | "xlsx" | "numbers" | "csv" | "tsv" | "ppt" | "pptx" | "key" | "epub" | "json"
            | "xml" | "html" | "htm" | "tex" | "log" | "yaml" | "yml" => Kind::Document,
            "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "dmg" | "pkg" | "mpkg"
            | "iso" | "zst" => Kind::Archive,
            "app" => Kind::App,
            _ => Kind::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Image => "Images",
            Kind::Video => "Videos",
            Kind::Music => "Music",
            Kind::Document => "Documents",
            Kind::Archive => "Archives",
            Kind::App => "Apps",
            Kind::Other => "Other",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Kind::Image => ph::IMAGE,
            Kind::Video => ph::PLAY_CIRCLE,
            Kind::Music => ph::MUSIC_NOTE,
            Kind::Document => ph::FILE_TEXT,
            Kind::Archive => ph::ARCHIVE,
            Kind::App => ph::SQUARES_FOUR,
            Kind::Other => ph::FILE,
        }
    }
}

/// Where a downloaded file came from, derived from its `kMDItemWhereFroms` attribute.
#[derive(Clone, Debug)]
pub struct Source {
    /// Stable grouping key, e.g. `gmail` or `example.com`.
    pub key: String,
    /// Human friendly name, e.g. `Gmail`.
    pub label: String,
    /// The URLs macOS recorded for this download.
    pub urls: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    /// Lower-case extension without the dot.
    pub ext: String,
    pub size: u64,
    /// Unix seconds.
    pub modified: i64,
    /// Unix seconds (file birth time, which is also the download time for downloads).
    pub added: i64,
    /// Unix seconds; when the file was last opened (from `com.apple.lastuseddate#PS`), if known.
    pub last_used: Option<i64>,
    pub kind: Kind,
    pub is_screenshot: bool,
    pub in_downloads: bool,
    pub source: Option<Source>,
    pub dims: Option<(u32, u32)>,
}

impl FileEntry {
    /// The date people think of as "when I got this file".
    pub fn date(&self) -> i64 {
        self.added.max(self.modified)
    }

    /// The last time the file was touched in any way we can see.
    pub fn last_touched(&self) -> i64 {
        self.last_used.unwrap_or(0).max(self.date())
    }

    pub fn type_label(&self) -> String {
        match self.kind {
            Kind::App => "App".into(),
            _ if self.ext.is_empty() => "File".into(),
            _ => match self.ext.as_str() {
                "jpg" => "JPEG".into(),
                e => e.to_uppercase(),
            },
        }
    }

    pub fn parent_display(&self) -> String {
        self.path
            .parent()
            .map(tildify)
            .unwrap_or_default()
    }
}

/// Everything the scanner found, plus a few cached aggregates.
#[derive(Default)]
pub struct Library {
    pub files: Vec<FileEntry>,
    pub home: PathBuf,
    pub scanned_at: i64,
}

impl Library {
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    pub fn remove_paths(&mut self, removed: &std::collections::HashSet<PathBuf>) {
        self.files.retain(|f| !removed.contains(&f.path));
    }
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// `/Users/me/Downloads` → `~/Downloads`
pub fn tildify(p: &Path) -> String {
    let home = home_dir();
    match p.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

pub fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

pub const DAY: i64 = 86_400;

/// Human readable size using decimal units like Finder (1 KB = 1000 bytes).
pub fn fmt_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} bytes");
    }
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i <= 1 || v >= 100.0 {
        format!("{:.0} {}", v, UNITS[i])
    } else if v >= 10.0 {
        format!("{:.1} {}", v, UNITS[i])
    } else {
        format!("{:.2} {}", v, UNITS[i])
            .replace(".00 ", " ")
    }
}

pub fn fmt_count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn fmt_date(secs: i64) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(secs, 0).single() {
        Some(dt) => dt.format("%b %-d, %Y at %-I:%M %p").to_string(),
        None => "—".into(),
    }
}

pub fn fmt_date_short(secs: i64) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(secs, 0).single() {
        Some(dt) => dt.format("%b %-d, %Y").to_string(),
        None => "—".into(),
    }
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{} {many}", fmt_count(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(fmt_size(512), "512 bytes");
        assert_eq!(fmt_size(776_000), "776 KB");
        assert_eq!(fmt_size(13_700_000), "13.7 MB");
        assert_eq!(fmt_size(2_870_000_000), "2.87 GB");
        assert_eq!(fmt_count(2469), "2,469");
    }
}
