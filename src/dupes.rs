//! Exact duplicate detection.
//!
//! 1. Group files by size (free, no I/O).
//! 2. For groups with 2+ files, hash the first 64 KB to split obvious non-matches.
//! 3. SHA-256 the full contents of what is left.
//!
//! Everything runs on the local machine in a background thread.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use sha2::{Digest, Sha256};

use crate::model::{FileEntry, Kind};

#[derive(Clone, Debug)]
pub struct DupGroup {
    pub size: u64,
    pub paths: Vec<PathBuf>,
    /// Index into `paths` of the copy we keep.
    pub keep: usize,
}

impl DupGroup {
    pub fn reclaimable(&self) -> u64 {
        self.size * (self.paths.len().saturating_sub(1)) as u64
    }
}

pub fn total_reclaimable(groups: &[DupGroup]) -> u64 {
    groups.iter().map(DupGroup::reclaimable).sum()
}

pub fn extra_copies(groups: &[DupGroup]) -> usize {
    groups.iter().map(|g| g.paths.len() - 1).sum()
}

fn hash_file(path: &Path, limit: Option<u64>) -> Option<[u8; 32]> {
    let f = File::open(path).ok()?;
    let mut r = BufReader::with_capacity(256 * 1024, f);
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut read_total = 0u64;
    loop {
        let want = match limit {
            Some(l) if read_total >= l => break,
            Some(l) => ((l - read_total) as usize).min(buf.len()),
            None => buf.len(),
        };
        let n = r.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        read_total += n as u64;
    }
    Some(h.finalize().into())
}

pub fn find(files: &[FileEntry]) -> Vec<DupGroup> {
    // Apps are bundles (folders); skip them and tiny files.
    let mut by_size: HashMap<u64, Vec<&FileEntry>> = HashMap::new();
    for f in files.iter().filter(|f| f.kind != Kind::App && f.size >= 1024) {
        by_size.entry(f.size).or_default().push(f);
    }
    let candidates: Vec<Vec<&FileEntry>> = by_size.into_values().filter(|v| v.len() > 1).collect();

    let mut groups: Vec<DupGroup> = candidates
        .par_iter()
        .flat_map_iter(|same_size| {
            let size = same_size[0].size;
            // Stage 2: partial hash
            let mut partial: HashMap<[u8; 32], Vec<&FileEntry>> = HashMap::new();
            for f in same_size {
                if let Some(h) = hash_file(&f.path, Some(64 * 1024)) {
                    partial.entry(h).or_default().push(f);
                }
            }
            // Stage 3: full hash (skip if the file is smaller than the partial window)
            let mut out = Vec::new();
            for (_, list) in partial.into_iter().filter(|(_, v)| v.len() > 1) {
                let mut full: HashMap<[u8; 32], Vec<&FileEntry>> = HashMap::new();
                if size <= 64 * 1024 {
                    full.insert([0; 32], list);
                } else {
                    for f in list {
                        if let Some(h) = hash_file(&f.path, None) {
                            full.entry(h).or_default().push(f);
                        }
                    }
                }
                for (_, same) in full.into_iter().filter(|(_, v)| v.len() > 1) {
                    out.push(make_group(size, same));
                }
            }
            out
        })
        .collect();

    groups.sort_by(|a, b| b.reclaimable().cmp(&a.reclaimable()));
    groups
}

/// Pick the copy to keep: prefer files outside Downloads, then names without
/// "copy" / "(1)", then the oldest one.
fn make_group(size: u64, mut files: Vec<&FileEntry>) -> DupGroup {
    files.sort_by_key(|f| f.path.clone());
    let score = |f: &FileEntry| {
        let n = f.name.to_lowercase();
        let copyish = n.contains(" copy") || n.contains("(1)") || n.contains("(2)") || n.contains("-1.") || n.contains(" 2.");
        (f.in_downloads as u8, copyish as u8, f.added)
    };
    let keep = files
        .iter()
        .enumerate()
        .min_by_key(|(_, f)| score(f))
        .map(|(i, _)| i)
        .unwrap_or(0);
    DupGroup { size, paths: files.iter().map(|f| f.path.clone()).collect(), keep }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_identical_files() {
        let dir = std::env::temp_dir().join(format!("hinder-dupes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let data = vec![7u8; 100_000];
        let mut other = data.clone();
        other[99_999] = 1;
        for (name, bytes) in [("a.bin", &data), ("b copy.bin", &data), ("c.bin", &other)] {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        let entries: Vec<FileEntry> = ["a.bin", "b copy.bin", "c.bin"]
            .iter()
            .map(|n| FileEntry {
                path: dir.join(n),
                name: n.to_string(),
                ext: "bin".into(),
                size: 100_000,
                modified: 0,
                added: 0,
                last_used: None,
                kind: Kind::Other,
                is_screenshot: false,
                in_downloads: false,
                source: None,
                dims: None,
            })
            .collect();
        let groups = find(&entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].paths.len(), 2);
        assert_eq!(groups[0].paths[groups[0].keep].file_name().unwrap(), "a.bin");
        std::fs::remove_dir_all(dir).ok();
    }
}
