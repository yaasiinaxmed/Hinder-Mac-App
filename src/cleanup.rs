//! Clean Up suggestions. Each rule is transparent and conservative; the user
//! always reviews the list, and files only ever go to the Trash.

use std::path::PathBuf;

use egui_phosphor::regular as ph;

use crate::dupes::{self, DupGroup};
use crate::model::{DAY, FileEntry, Kind, Library, now_secs};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardKind {
    Duplicates,
    OldInstallers,
    ForgottenDownloads,
    OldScreenshots,
    BigOldVideos,
    UnusedApps,
}

impl CardKind {
    pub const ALL: [CardKind; 6] = [
        CardKind::Duplicates,
        CardKind::OldInstallers,
        CardKind::ForgottenDownloads,
        CardKind::OldScreenshots,
        CardKind::BigOldVideos,
        CardKind::UnusedApps,
    ];

    pub fn title(self) -> &'static str {
        match self {
            CardKind::Duplicates => "Duplicates",
            CardKind::OldInstallers => "Old Installers",
            CardKind::ForgottenDownloads => "Forgotten Downloads",
            CardKind::OldScreenshots => "Old Screenshots",
            CardKind::BigOldVideos => "Big Old Videos",
            CardKind::UnusedApps => "Unused Apps",
        }
    }

    pub fn explain(self) -> &'static str {
        match self {
            CardKind::Duplicates => "Extra copies of files that are byte-for-byte identical. One copy of each is kept.",
            CardKind::OldInstallers => ".dmg and .pkg installers older than a week. Once an app is installed you rarely need these.",
            CardKind::ForgottenDownloads => "Files in Downloads you haven't opened in over 3 months.",
            CardKind::OldScreenshots => "Screenshots older than 30 days.",
            CardKind::BigOldVideos => "Videos over 200 MB you haven't touched in 6 months.",
            CardKind::UnusedApps => "Apps you haven't opened in over 3 months.",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            CardKind::Duplicates => ph::COPY,
            CardKind::OldInstallers => ph::PACKAGE,
            CardKind::ForgottenDownloads => ph::DOWNLOAD_SIMPLE,
            CardKind::OldScreenshots => ph::SCAN,
            CardKind::BigOldVideos => ph::FILM_STRIP,
            CardKind::UnusedApps => ph::APP_WINDOW,
        }
    }

    /// Palette slot used for tinting in the Vivid look.
    pub fn tint_index(self) -> usize {
        match self {
            CardKind::Duplicates => 5,
            CardKind::OldInstallers => 4,
            CardKind::ForgottenDownloads => 1,
            CardKind::OldScreenshots => 0,
            CardKind::BigOldVideos => 2,
            CardKind::UnusedApps => 3,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Card {
    pub kind: CardKind,
    /// Files suggested for the Trash.
    pub paths: Vec<PathBuf>,
    /// Size of each entry in `paths`.
    pub sizes: Vec<u64>,
    pub bytes: u64,
}

impl Card {
    pub fn count_label(&self) -> String {
        let n = self.paths.len();
        match self.kind {
            CardKind::Duplicates => format!("{} extra cop{}", crate::model::fmt_count(n), if n == 1 { "y" } else { "ies" }),
            _ => crate::model::plural(n, "item", "items"),
        }
    }
}

pub fn compute(lib: &Library, dups: &[DupGroup]) -> Vec<Card> {
    let now = now_secs();
    let mut cards = Vec::new();

    let (dup_paths, dup_sizes): (Vec<PathBuf>, Vec<u64>) = dups
        .iter()
        .flat_map(|g| g.paths.iter().enumerate().filter(move |(i, _)| *i != g.keep).map(move |(_, p)| (p.clone(), g.size)))
        .unzip();
    cards.push(Card { kind: CardKind::Duplicates, paths: dup_paths, sizes: dup_sizes, bytes: dupes::total_reclaimable(dups) });

    let rule = |kind: CardKind, pred: &dyn Fn(&FileEntry) -> bool| {
        let mut hits: Vec<&FileEntry> = lib.files.iter().filter(|f| pred(f)).collect();
        hits.sort_by(|a, b| b.size.cmp(&a.size));
        Card {
            kind,
            bytes: hits.iter().map(|f| f.size).sum(),
            sizes: hits.iter().map(|f| f.size).collect(),
            paths: hits.into_iter().map(|f| f.path.clone()).collect(),
        }
    };

    cards.push(rule(CardKind::OldInstallers, &|f| {
        matches!(f.ext.as_str(), "dmg" | "pkg" | "mpkg") && now - f.date() > 7 * DAY
    }));
    cards.push(rule(CardKind::ForgottenDownloads, &|f| {
        f.in_downloads && f.kind != Kind::App && !matches!(f.ext.as_str(), "dmg" | "pkg" | "mpkg") && now - f.last_touched() > 90 * DAY
    }));
    cards.push(rule(CardKind::OldScreenshots, &|f| f.is_screenshot && now - f.date() > 30 * DAY));
    cards.push(rule(CardKind::BigOldVideos, &|f| {
        f.kind == Kind::Video && f.size > 200_000_000 && now - f.last_touched() > 180 * DAY
    }));
    cards.push(rule(CardKind::UnusedApps, &|f| {
        // Only judge apps we have a "last opened" date for; never guess.
        f.kind == Kind::App && f.path.starts_with("/Applications") && f.last_used.is_some_and(|t| now - t > 90 * DAY)
    }));

    cards
}

/// Total across all cards, counting each file once.
pub fn total(cards: &[Card]) -> u64 {
    let mut seen = std::collections::HashSet::new();
    cards
        .iter()
        .flat_map(|c| c.paths.iter().zip(&c.sizes))
        .filter(|(p, _)| seen.insert((*p).clone()))
        .map(|(_, s)| *s)
        .sum()
}
