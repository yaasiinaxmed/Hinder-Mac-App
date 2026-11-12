//! Thumbnail pipeline.
//!
//! Thumbnails come from macOS Quick Look (`qlmanage -t`), which understands
//! images, HEIC, PDFs, videos, Office files and app icons. Results are cached in
//! `~/Library/Caches/Hinder/thumbs` and uploaded to the GPU on demand. Requests
//! are served newest-first so whatever is on screen right now loads first.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Condvar, Mutex, mpsc};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

const THUMB_PX: u32 = 360;
const MAX_TEXTURES: usize = 700;
const WORKERS: usize = 6;

enum Slot {
    Pending,
    Ready { tex: TextureHandle, last_frame: u64 },
    Failed,
}

struct Queue {
    stack: Mutex<Vec<PathBuf>>,
    cv: Condvar,
}

pub struct Thumbs {
    slots: HashMap<PathBuf, Slot>,
    queue: Arc<Queue>,
    rx: mpsc::Receiver<(PathBuf, Option<ColorImage>)>,
    frame: u64,
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("Hinder").join("thumbs")
}

impl Thumbs {
    pub fn new(ctx: &egui::Context) -> Self {
        let queue = Arc::new(Queue { stack: Mutex::new(Vec::new()), cv: Condvar::new() });
        let (tx, rx) = mpsc::channel();
        let dir = cache_dir();
        let _ = std::fs::create_dir_all(&dir);
        for i in 0..WORKERS {
            let queue = queue.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            let dir = dir.clone();
            std::thread::Builder::new()
                .name(format!("hinder-thumb-{i}"))
                .spawn(move || loop {
                    let path = {
                        let mut s = queue.stack.lock().unwrap();
                        while s.is_empty() {
                            s = queue.cv.wait(s).unwrap();
                        }
                        s.pop().unwrap()
                    };
                    let img = make_thumb(&path, &dir);
                    if tx.send((path, img)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                })
                .expect("spawn thumb worker");
        }
        Self { slots: HashMap::new(), queue, rx, frame: 0 }
    }

    /// Call once per frame: uploads finished thumbnails and evicts old ones.
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        let mut budget = 24; // uploads per frame, keeps scrolling smooth
        while budget > 0 {
            let Ok((path, img)) = self.rx.try_recv() else { break };
            budget -= 1;
            let slot = match img {
                Some(img) => {
                    let name = path.to_string_lossy().to_string();
                    Slot::Ready { tex: ctx.load_texture(name, img, TextureOptions::LINEAR), last_frame: self.frame }
                }
                None => Slot::Failed,
            };
            self.slots.insert(path, slot);
        }
        if budget == 0 {
            ctx.request_repaint();
        }

        let ready = self.slots.values().filter(|s| matches!(s, Slot::Ready { .. })).count();
        if ready > MAX_TEXTURES {
            let mut ages: Vec<(u64, PathBuf)> = self
                .slots
                .iter()
                .filter_map(|(p, s)| match s {
                    Slot::Ready { last_frame, .. } => Some((*last_frame, p.clone())),
                    _ => None,
                })
                .collect();
            ages.sort();
            for (_, p) in ages.into_iter().take(ready - MAX_TEXTURES + 100) {
                self.slots.remove(&p);
            }
        }
        // Forget queued work that scrolled away long ago.
        let mut s = self.queue.stack.lock().unwrap();
        if s.len() > 400 {
            let drop_n = s.len() - 400;
            for p in s.drain(..drop_n) {
                self.slots.remove(&p);
            }
        }
    }

    /// Returns the texture if ready; otherwise queues the work and returns `None`.
    pub fn get(&mut self, path: &Path) -> Option<&TextureHandle> {
        let frame = self.frame;
        if !self.slots.contains_key(path) {
            self.slots.insert(path.to_path_buf(), Slot::Pending);
            self.queue.stack.lock().unwrap().push(path.to_path_buf());
            self.queue.cv.notify_one();
            return None;
        }
        match self.slots.get_mut(path) {
            Some(Slot::Ready { tex, last_frame }) => {
                *last_frame = frame;
                Some(tex)
            }
            _ => None,
        }
    }

    pub fn failed(&self, path: &Path) -> bool {
        matches!(self.slots.get(path), Some(Slot::Failed))
    }

    pub fn forget(&mut self, path: &Path) {
        self.slots.remove(path);
    }
}

fn cache_key(path: &Path) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    if let Ok(m) = std::fs::metadata(path) {
        m.len().hash(&mut h);
        m.modified().ok().hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

fn make_thumb(path: &Path, dir: &Path) -> Option<ColorImage> {
    let cached = dir.join(format!("{}.png", cache_key(path)));
    if !cached.exists() {
        let tmp = dir.join(format!("tmp-{:?}", std::thread::current().id()).replace(['(', ')'], ""));
        let _ = std::fs::create_dir_all(&tmp);
        let status = Command::new("/usr/bin/qlmanage")
            .args(["-t", "-s", &THUMB_PX.to_string(), "-o"])
            .arg(&tmp)
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
        // qlmanage writes "<file name>.png" into the output folder.
        let produced = std::fs::read_dir(&tmp).ok()?.flatten().find(|e| e.path().extension().is_some_and(|x| x == "png"))?;
        std::fs::rename(produced.path(), &cached).ok()?;
    }
    let img = image::open(&cached).ok()?.into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}
