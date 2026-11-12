//! Plain-English search.
//!
//! `"pdfs from last week over 5 mb"` becomes a [`Query`] with a kind, a date range
//! and a size limit. Everything the parser understood is shown back to the user
//! as chips, so there are no surprises, and every chip can be removed or changed.

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};

use crate::model::{DAY, FileEntry, Kind, fmt_size, home_dir};
use crate::sources;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DateFilter {
    #[default]
    Any,
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    ThisYear,
    LastYear,
    /// Within the last N days.
    LastDays(u32),
    /// Older than N days.
    OlderThan(u32),
    Month { month: u32, year: i32 },
    Year(i32),
}

impl DateFilter {
    pub const PRESETS: [DateFilter; 8] = [
        DateFilter::Any,
        DateFilter::Today,
        DateFilter::Yesterday,
        DateFilter::LastDays(7),
        DateFilter::LastDays(30),
        DateFilter::ThisYear,
        DateFilter::OlderThan(180),
        DateFilter::OlderThan(365),
    ];

    pub fn label(&self) -> String {
        const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
        match *self {
            DateFilter::Any => "Any".into(),
            DateFilter::Today => "Today".into(),
            DateFilter::Yesterday => "Yesterday".into(),
            DateFilter::ThisWeek => "This week".into(),
            DateFilter::LastWeek => "Last week".into(),
            DateFilter::ThisMonth => "This month".into(),
            DateFilter::LastMonth => "Last month".into(),
            DateFilter::ThisYear => "This year".into(),
            DateFilter::LastYear => "Last year".into(),
            DateFilter::LastDays(7) => "Past 7 days".into(),
            DateFilter::LastDays(30) => "Past 30 days".into(),
            DateFilter::LastDays(n) => format!("Past {} days", n),
            DateFilter::OlderThan(n) if n % 365 == 0 => {
                let y = n / 365;
                format!("Older than {} year{}", y, if y == 1 { "" } else { "s" })
            }
            DateFilter::OlderThan(n) if n % 30 == 0 => {
                let m = n / 30;
                format!("Older than {} month{}", m, if m == 1 { "" } else { "s" })
            }
            DateFilter::OlderThan(n) => format!("Older than {} days", n),
            DateFilter::Month { month, year } => format!("{} {}", MONTHS[(month - 1) as usize], year),
            DateFilter::Year(y) => y.to_string(),
        }
    }

    /// Inclusive start / exclusive end, as Unix seconds.
    pub fn range(&self) -> (Option<i64>, Option<i64>) {
        let now = Local::now();
        let today = now.date_naive();
        let start_of = |d: NaiveDate| Local.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap()).earliest().map(|t| t.timestamp());
        let week_start = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        let month_start = today.with_day(1).unwrap();
        let ym = |y: i32, m: u32| NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        let next_month = |y: i32, m: u32| if m == 12 { ym(y + 1, 1) } else { ym(y, m + 1) };
        match *self {
            DateFilter::Any => (None, None),
            DateFilter::Today => (start_of(today), None),
            DateFilter::Yesterday => (start_of(today - Duration::days(1)), start_of(today)),
            DateFilter::ThisWeek => (start_of(week_start), None),
            DateFilter::LastWeek => (start_of(week_start - Duration::days(7)), start_of(week_start)),
            DateFilter::ThisMonth => (start_of(month_start), None),
            DateFilter::LastMonth => {
                let prev = (month_start - Duration::days(1)).with_day(1).unwrap();
                (start_of(prev), start_of(month_start))
            }
            DateFilter::ThisYear => (start_of(ym(today.year(), 1)), None),
            DateFilter::LastYear => (start_of(ym(today.year() - 1, 1)), start_of(ym(today.year(), 1))),
            DateFilter::LastDays(n) => (Some(now.timestamp() - n as i64 * DAY), None),
            DateFilter::OlderThan(n) => (None, Some(now.timestamp() - n as i64 * DAY)),
            DateFilter::Month { month, year } => (start_of(ym(year, month)), start_of(next_month(year, month))),
            DateFilter::Year(y) => (start_of(ym(y, 1)), start_of(ym(y + 1, 1))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SizeFilter {
    pub min: Option<u64>,
    pub max: Option<u64>,
}

impl SizeFilter {
    pub const PRESETS: [(&'static str, SizeFilter); 6] = [
        ("Any", SizeFilter { min: None, max: None }),
        ("Under 1 MB", SizeFilter { min: None, max: Some(1_000_000) }),
        ("1 – 100 MB", SizeFilter { min: Some(1_000_000), max: Some(100_000_000) }),
        ("Over 100 MB", SizeFilter { min: Some(100_000_000), max: None }),
        ("Over 500 MB", SizeFilter { min: Some(500_000_000), max: None }),
        ("Over 1 GB", SizeFilter { min: Some(1_000_000_000), max: None }),
    ];

    pub fn is_any(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    pub fn label(&self) -> String {
        match (self.min, self.max) {
            (None, None) => "Any".into(),
            (Some(a), None) => format!("Over {}", fmt_size(a)),
            (None, Some(b)) => format!("Under {}", fmt_size(b)),
            (Some(a), Some(b)) => format!("{} – {}", fmt_size(a), fmt_size(b)),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    pub kinds: BTreeSet<Kind>,
    pub exts: BTreeSet<String>,
    pub screenshots: bool,
    /// Photos taken on an iPhone / iPad (IMG_1234.HEIC style names).
    pub camera: bool,
    pub date: DateFilter,
    pub size: SizeFilter,
    pub source: Option<String>,
    pub location: Option<PathBuf>,
    /// Free text that must appear in the name, the path or (via Spotlight) the contents.
    pub text: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Facet {
    Kind(Kind),
    Ext(String),
    Screenshots,
    Camera,
    Date,
    Size,
    Source,
    Location,
    Text,
}

pub struct Chip {
    pub label: String,
    pub facet: Facet,
}

impl Query {
    pub fn is_empty(&self) -> bool {
        *self == Query::default()
    }

    pub fn text_phrase(&self) -> String {
        self.text.join(" ")
    }

    pub fn matches(&self, f: &FileEntry, content_hits: Option<&HashSet<PathBuf>>) -> bool {
        let type_ok = (self.kinds.is_empty() && self.exts.is_empty()) || self.kinds.contains(&f.kind) || self.exts.contains(&f.ext);
        if !type_ok {
            return false;
        }
        if self.screenshots && !f.is_screenshot {
            return false;
        }
        if self.camera && !(f.kind == Kind::Image || f.kind == Kind::Video) {
            return false;
        }
        if self.camera && !is_camera_name(&f.name) {
            return false;
        }
        if let Some(min) = self.size.min {
            if f.size < min {
                return false;
            }
        }
        if let Some(max) = self.size.max {
            if f.size > max {
                return false;
            }
        }
        if self.date != DateFilter::Any {
            let (a, b) = self.date.range();
            let d = f.date();
            if a.is_some_and(|a| d < a) || b.is_some_and(|b| d >= b) {
                return false;
            }
        }
        if let Some(src) = &self.source {
            match &f.source {
                Some(s) if &s.key == src => {}
                _ => return false,
            }
        }
        if let Some(loc) = &self.location {
            if !f.path.starts_with(loc) {
                return false;
            }
        }
        if !self.text.is_empty() {
            let name = f.name.to_lowercase();
            let path = f.path.to_string_lossy().to_lowercase();
            let in_name = self.text.iter().all(|t| name.contains(t.as_str()) || path.contains(t.as_str()));
            let in_content = content_hits.is_some_and(|h| h.contains(&f.path));
            if !in_name && !in_content {
                return false;
            }
        }
        true
    }

    pub fn chips(&self, source_label: impl Fn(&str) -> String) -> Vec<Chip> {
        let mut out = Vec::new();
        for k in &self.kinds {
            out.push(Chip { label: k.label().into(), facet: Facet::Kind(*k) });
        }
        for e in &self.exts {
            let label = match e.as_str() {
                "pdf" => "PDFs".to_string(),
                "dmg" | "pkg" => format!("{} installers", e.to_uppercase()),
                _ => format!("{} files", e.to_uppercase()),
            };
            out.push(Chip { label, facet: Facet::Ext(e.clone()) });
        }
        if self.screenshots {
            out.push(Chip { label: "Screenshots".into(), facet: Facet::Screenshots });
        }
        if self.camera {
            out.push(Chip { label: "From a camera / iPhone".into(), facet: Facet::Camera });
        }
        if self.date != DateFilter::Any {
            out.push(Chip { label: self.date.label(), facet: Facet::Date });
        }
        if !self.size.is_any() {
            out.push(Chip { label: self.size.label(), facet: Facet::Size });
        }
        if let Some(s) = &self.source {
            out.push(Chip { label: format!("From {}", source_label(s)), facet: Facet::Source });
        }
        if let Some(l) = &self.location {
            out.push(Chip { label: format!("In {}", l.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), facet: Facet::Location });
        }
        if !self.text.is_empty() {
            out.push(Chip { label: format!("“{}”", self.text_phrase()), facet: Facet::Text });
        }
        out
    }

    pub fn remove(&mut self, facet: &Facet) {
        match facet {
            Facet::Kind(k) => {
                self.kinds.remove(k);
            }
            Facet::Ext(e) => {
                self.exts.remove(e);
            }
            Facet::Screenshots => self.screenshots = false,
            Facet::Camera => self.camera = false,
            Facet::Date => self.date = DateFilter::Any,
            Facet::Size => self.size = SizeFilter::default(),
            Facet::Source => self.source = None,
            Facet::Location => self.location = None,
            Facet::Text => self.text.clear(),
        }
    }
}

fn is_camera_name(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    (n.starts_with("IMG_") || n.starts_with("IMG-") || n.starts_with("DSC") || n.starts_with("PXL_") || n.starts_with("MVIMG"))
        && n[4.min(n.len())..].chars().next().is_some_and(|c| c.is_ascii_digit() || c == '_' || c == 'E')
}

const STOP: &[&str] = &[
    "a", "an", "the", "my", "all", "any", "some", "files", "file", "of", "that", "which", "show", "me", "find", "get", "and", "or",
    "with", "containing", "contains", "about", "where", "is", "are", "was", "were", "i", "got", "received", "downloaded",
    "saved", "made", "created", "taken", "to", "for", "please", "stuff", "things", "items", "folder", "than", "ago", "from",
    "in", "on", "at", "by", "via", "into", "named", "called", "text", "say", "says", "saying",
];

fn kind_word(w: &str) -> Option<(Option<Kind>, Option<&'static [&'static str]>, bool)> {
    // (kind, extensions, screenshot)
    Some(match w {
        "image" | "images" | "photo" | "photos" | "picture" | "pictures" | "pic" | "pics" | "img" | "imgs" => (Some(Kind::Image), None, false),
        "screenshot" | "screenshots" | "screencap" | "screencaps" | "screengrab" | "screengrabs" => (None, None, true),
        "video" | "videos" | "movie" | "movies" | "clip" | "clips" | "recording" | "recordings" | "film" => (Some(Kind::Video), None, false),
        "music" | "song" | "songs" | "audio" | "track" | "tracks" | "podcast" | "podcasts" => (Some(Kind::Music), None, false),
        "doc" | "docs" | "document" | "documents" | "paper" | "papers" => (Some(Kind::Document), None, false),
        "archive" | "archives" | "compressed" => (Some(Kind::Archive), None, false),
        "app" | "apps" | "application" | "applications" | "program" | "programs" => (Some(Kind::App), None, false),
        "pdf" | "pdfs" => (None, Some(&["pdf"]), false),
        "zip" | "zips" | "zipped" => (None, Some(&["zip", "rar", "7z", "tar", "gz", "tgz"]), false),
        "installer" | "installers" | "dmg" | "dmgs" | "pkg" | "pkgs" => (None, Some(&["dmg", "pkg", "mpkg"]), false),
        "spreadsheet" | "spreadsheets" | "excel" | "sheet" | "sheets" | "xlsx" | "csv" | "csvs" => (None, Some(&["xlsx", "xls", "csv", "numbers", "tsv"]), false),
        "presentation" | "presentations" | "slides" | "deck" | "decks" | "pptx" | "keynote" => (None, Some(&["pptx", "ppt", "key"]), false),
        "word" | "docx" => (None, Some(&["docx", "doc", "pages", "rtf"]), false),
        "gif" | "gifs" => (None, Some(&["gif"]), false),
        "png" | "pngs" => (None, Some(&["png"]), false),
        "jpg" | "jpgs" | "jpeg" | "jpegs" => (None, Some(&["jpg", "jpeg"]), false),
        "heic" => (None, Some(&["heic", "heif"]), false),
        "svg" | "svgs" => (None, Some(&["svg"]), false),
        "psd" => (None, Some(&["psd"]), false),
        "mp3" | "mp3s" => (None, Some(&["mp3"]), false),
        "wav" => (None, Some(&["wav"]), false),
        "mp4" | "mp4s" => (None, Some(&["mp4"]), false),
        "mov" | "movs" => (None, Some(&["mov"]), false),
        "txt" => (None, Some(&["txt"]), false),
        "md" | "markdown" => (None, Some(&["md", "markdown"]), false),
        "epub" | "ebook" | "ebooks" | "book" | "books" => (None, Some(&["epub", "pdf", "mobi"]), false),
        "font" | "fonts" => (None, Some(&["ttf", "otf", "woff", "woff2"]), false),
        "code" => (None, Some(&["rs", "js", "ts", "py", "swift", "go", "java", "c", "cpp", "h", "rb"]), false),
        _ => return None,
    })
}

fn month_num(w: &str) -> Option<u32> {
    let full = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];
    for (i, f) in full.iter().enumerate() {
        if w == *f || (w.len() == 3 && f.starts_with(w)) || (w == "sept" && i == 8) {
            return Some(i as u32 + 1);
        }
    }
    None
}

fn unit_days(w: &str) -> Option<u32> {
    Some(match w.trim_end_matches('s') {
        "day" | "d" => 1,
        "week" | "wk" | "w" => 7,
        "month" | "mo" | "mon" => 30,
        "year" | "yr" | "y" => 365,
        _ => return None,
    })
}

fn word_number(w: &str) -> Option<f64> {
    if let Ok(v) = w.parse::<f64>() {
        return Some(v);
    }
    Some(match w {
        "a" | "an" | "one" => 1.0,
        "two" | "couple" => 2.0,
        "three" | "few" => 3.0,
        "four" => 4.0,
        "five" => 5.0,
        "six" => 6.0,
        "seven" => 7.0,
        "eight" => 8.0,
        "nine" => 9.0,
        "ten" => 10.0,
        "twelve" => 12.0,
        _ => return None,
    })
}

fn size_unit(w: &str) -> Option<f64> {
    Some(match w {
        "b" | "byte" | "bytes" => 1.0,
        "k" | "kb" | "kbs" | "kilobyte" | "kilobytes" => 1e3,
        "m" | "mb" | "mbs" | "meg" | "megs" | "megabyte" | "megabytes" => 1e6,
        "g" | "gb" | "gbs" | "gig" | "gigs" | "gigabyte" | "gigabytes" => 1e9,
        "t" | "tb" | "terabyte" | "terabytes" => 1e12,
        _ => return None,
    })
}

/// Split "5mb" → ["5", "mb"], "1.5gb" → ["1.5", "gb"], strip punctuation.
fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in s.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '!' | '?' | '"' | '“' | '”' | '(' | ')')) {
        let w = raw.trim_matches(|c: char| matches!(c, '.' | '\'' | '’'));
        if w.is_empty() {
            continue;
        }
        if w == ">" || w == "<" {
            out.push(w.to_string());
            continue;
        }
        let digits: String = w.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if !digits.is_empty() && digits.len() < w.len() && size_unit(&w[digits.len()..]).is_some() {
            let unit = w[digits.len()..].to_string();
            out.push(digits);
            out.push(unit);
        } else if let Some(rest) = w.strip_prefix('>').or_else(|| w.strip_prefix('<')) {
            out.push(w[..1].to_string());
            out.extend(tokenize(rest));
        } else {
            out.push(w.to_string());
        }
    }
    out
}

/// Known locations people mention ("in downloads", "on my desktop").
fn location_word(w: &str) -> Option<PathBuf> {
    let home = home_dir();
    Some(match w {
        "downloads" | "download" => home.join("Downloads"),
        "desktop" => home.join("Desktop"),
        "documents" => home.join("Documents"),
        "pictures" => home.join("Pictures"),
        "movies" => home.join("Movies"),
        "music" => home.join("Music"),
        _ => return None,
    })
}

/// Parse a plain-English query. `known_sources` are the grouping keys present in
/// the library, so "from pixabay.com" or "from canva" can be recognised.
pub fn parse(input: &str, known_sources: &[(String, String)]) -> Query {
    let toks = tokenize(&input.to_lowercase());
    let mut q = Query::default();
    let n = toks.len();
    let t = |i: usize| toks.get(i).map(String::as_str).unwrap_or("");
    let mut i = 0;
    while i < n {
        let w = t(i);

        // ---------- locations: "in downloads", "on my desktop" ----------
        if matches!(w, "in" | "on" | "inside") {
            let j = if t(i + 1) == "my" || t(i + 1) == "the" { i + 2 } else { i + 1 };
            if let Some(loc) = location_word(t(j)) {
                q.location = Some(loc);
                i = j + 1;
                if t(i) == "folder" {
                    i += 1;
                }
                continue;
            }
        }

        // ---------- sources: "from gmail", "from pixabay.com" ----------
        if matches!(w, "from" | "via") && i + 1 < n {
            let j = if t(i + 1) == "my" || t(i + 1) == "the" { i + 2 } else { i + 1 };
            let cand = t(j);
            if matches!(cand, "iphone" | "ipad" | "camera" | "phone") {
                q.camera = true;
                i = j + 1;
                continue;
            }
            if let Some(key) = sources::alias_to_key(cand) {
                q.source = Some(key.to_string());
                i = j + 1;
                continue;
            }
            if let Some((key, _)) = known_sources.iter().find(|(k, l)| k == cand || l.to_lowercase() == cand || k.split('.').next() == Some(cand)) {
                q.source = Some(key.clone());
                i = j + 1;
                continue;
            }
            if let Some(loc) = location_word(cand) {
                q.location = Some(loc);
                i = j + 1;
                continue;
            }
        }
        if matches!(w, "iphone" | "ipad") {
            q.camera = true;
            i += 1;
            continue;
        }

        // ---------- dates ----------
        match (w, t(i + 1)) {
            ("today", _) => { q.date = DateFilter::Today; i += 1; continue; }
            ("yesterday", _) => { q.date = DateFilter::Yesterday; i += 1; continue; }
            ("this", "week") => { q.date = DateFilter::ThisWeek; i += 2; continue; }
            ("this", "month") => { q.date = DateFilter::ThisMonth; i += 2; continue; }
            ("this", "year") => { q.date = DateFilter::ThisYear; i += 2; continue; }
            ("last" | "previous" | "past", "week") => { q.date = DateFilter::LastWeek; i += 2; continue; }
            ("last" | "previous" | "past", "month") => { q.date = DateFilter::LastMonth; i += 2; continue; }
            ("last" | "previous" | "past", "year") => { q.date = DateFilter::LastYear; i += 2; continue; }
            ("recent" | "recently" | "new" | "newest" | "latest", _) => { q.date = DateFilter::LastDays(7); i += 1; continue; }
            _ => {}
        }
        // "last 3 days", "past two weeks", "within 10 days"
        if matches!(w, "last" | "past" | "within" | "previous") {
            if let (Some(num), Some(unit)) = (word_number(t(i + 1)), unit_days(t(i + 2))) {
                q.date = DateFilter::LastDays((num * unit as f64).round() as u32);
                i += 3;
                continue;
            }
        }
        // "older than 6 months", "more than a year old"
        if matches!(w, "older" | "before") && t(i + 1) == "than" {
            if let (Some(num), Some(unit)) = (word_number(t(i + 2)), unit_days(t(i + 3))) {
                q.date = DateFilter::OlderThan((num * unit as f64).round() as u32);
                i += 4;
                continue;
            }
        }
        if matches!(w, "newer" | "younger") && t(i + 1) == "than" {
            if let (Some(num), Some(unit)) = (word_number(t(i + 2)), unit_days(t(i + 3))) {
                q.date = DateFilter::LastDays((num * unit as f64).round() as u32);
                i += 4;
                continue;
            }
        }
        // "3 days ago", "2 weeks old"
        if let (Some(num), Some(unit)) = (word_number(w).filter(|_| w != "a" && w != "an"), unit_days(t(i + 1))) {
            if matches!(t(i + 2), "ago" | "old") {
                let days = (num * unit as f64).round() as u32;
                q.date = if t(i + 2) == "old" { DateFilter::OlderThan(days) } else { DateFilter::LastDays(days + unit) };
                i += 3;
                continue;
            }
        }
        if w == "old" || w == "older" {
            if q.date == DateFilter::Any {
                q.date = DateFilter::OlderThan(180);
            }
            i += 1;
            continue;
        }
        if let Some(m) = month_num(w) {
            let now = Local::now();
            let explicit_year = t(i + 1).parse::<i32>().ok().filter(|y| (1990..=2100).contains(y));
            let year = explicit_year.unwrap_or(if m as u32 > now.month() { now.year() - 1 } else { now.year() });
            q.date = DateFilter::Month { month: m, year };
            i += if explicit_year.is_some() { 2 } else { 1 };
            continue;
        }
        if let Ok(y) = w.parse::<i32>() {
            if (1990..=2100).contains(&y) && size_unit(t(i + 1)).is_none() {
                q.date = DateFilter::Year(y);
                i += 1;
                continue;
            }
        }

        // ---------- sizes ----------
        let cmp_min = matches!(w, "over" | "above" | "bigger" | "larger" | "greater" | "more" | ">" | "min" | "atleast" | "exceeding");
        let cmp_max = matches!(w, "under" | "below" | "smaller" | "less" | "<" | "max" | "lighter" | "within");
        if cmp_min || cmp_max {
            let mut j = i + 1;
            if t(j) == "than" {
                j += 1;
            }
            if t(j) == "a" || t(j) == "an" {
                // "bigger than a gig"
            }
            if let (Some(num), Some(unit)) = (word_number(t(j)), size_unit(t(j + 1))) {
                let bytes = (num * unit) as u64;
                if cmp_min { q.size.min = Some(bytes); } else { q.size.max = Some(bytes); }
                i = j + 2;
                continue;
            }
            if let Some(unit) = size_unit(t(j)) {
                // "over a gig" / "bigger than gb"
                if cmp_min { q.size.min = Some(unit as u64); } else { q.size.max = Some(unit as u64); }
                i = j + 1;
                continue;
            }
        }
        // bare "5 mb" → treat as "over 5 MB"
        if let (Some(num), Some(unit)) = (w.parse::<f64>().ok(), size_unit(t(i + 1))) {
            q.size.min = Some((num * unit) as u64);
            i += 2;
            continue;
        }
        match w {
            "big" | "large" | "biggest" | "largest" | "heavy" => { q.size.min = Some(100_000_000); i += 1; continue; }
            "huge" | "massive" | "giant" | "enormous" => { q.size.min = Some(1_000_000_000); i += 1; continue; }
            "small" | "tiny" | "little" => { q.size.max = Some(1_000_000); i += 1; continue; }
            _ => {}
        }

        // ---------- kinds ----------
        if let Some((kind, exts, shot)) = kind_word(w) {
            if let Some(k) = kind {
                q.kinds.insert(k);
            }
            if let Some(es) = exts {
                q.exts.extend(es.iter().map(|e| e.to_string()));
            }
            if shot {
                q.screenshots = true;
            }
            i += 1;
            continue;
        }

        // ---------- bare brand words: "gmail invoice" ----------
        if let Some(key) = sources::alias_to_key(w).filter(|_| !matches!(w, "mail" | "email" | "x" | "drive" | "gpt" | "extension")) {
            q.source = Some(key.to_string());
            i += 1;
            continue;
        }

        if !STOP.contains(&w) && !w.is_empty() {
            q.text.push(w.to_string());
        }
        i += 1;
    }
    // Screenshots are images; don't show both chips.
    if q.screenshots {
        q.kinds.remove(&Kind::Image);
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Query {
        parse(s, &[("pixabay.com".into(), "pixabay.com".into())])
    }

    #[test]
    fn pdfs_last_week_over_5mb() {
        let q = p("pdfs from last week over 5 mb");
        assert!(q.exts.contains("pdf"));
        assert_eq!(q.date, DateFilter::LastWeek);
        assert_eq!(q.size.min, Some(5_000_000));
        assert!(q.text.is_empty());
    }

    #[test]
    fn zips_bigger_older() {
        let q = p("zips bigger than 1gb older than 6 months");
        assert!(q.exts.contains("zip"));
        assert_eq!(q.size.min, Some(1_000_000_000));
        assert_eq!(q.date, DateFilter::OlderThan(180));
    }

    #[test]
    fn screenshots_with_text() {
        let q = p("screenshots with wifi password");
        assert!(q.screenshots);
        assert_eq!(q.text, vec!["wifi", "password"]);
    }

    #[test]
    fn iphone_august() {
        let q = p("videos from my iphone in august");
        assert!(q.kinds.contains(&Kind::Video));
        assert!(q.camera);
        assert!(matches!(q.date, DateFilter::Month { month: 8, .. }));
    }

    #[test]
    fn sources() {
        assert_eq!(p("pdfs from gmail").source.as_deref(), Some("gmail"));
        assert_eq!(p("music from pixabay").source.as_deref(), Some("pixabay.com"));
        assert_eq!(p("invoice in downloads").location, Some(home_dir().join("Downloads")));
    }

    #[test]
    fn plain_text() {
        let q = p("tax return 2024");
        assert_eq!(q.text, vec!["tax", "return"]);
        assert_eq!(q.date, DateFilter::Year(2024));
    }
}
