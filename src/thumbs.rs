//! Thumbnail pipeline.
//!
//! Thumbnails come from macOS Quick Look (`qlmanage -t`), which understands
//! images, HEIC, PDFs, videos, Office files and app icons. Results are cached in
//! `~/Library/Caches/Hinder/thumbs` and uploaded to the GPU on demand. Requests
//! are served newest-first so whatever is on screen right now loads first.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
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

/// Draws an app's real icon (the one Finder shows, including layered macOS 26
/// icons and Assets.car-only apps) into a PNG, using AppKit from JavaScript for
/// Automation. Quick Look returns nothing useful for `.app` bundles.
const APP_ICON_JXA: &str = r#"
ObjC.import('AppKit');
function run(argv) {
  var size = parseInt(argv[2]);
  var img = $.NSWorkspace.sharedWorkspace.iconForFile(argv[0]);
  var rep = $.NSBitmapImageRep.alloc.initWithBitmapDataPlanesPixelsWidePixelsHighBitsPerSampleSamplesPerPixelHasAlphaIsPlanarColorSpaceNameBytesPerRowBitsPerPixel(null, size, size, 8, 4, true, false, $.NSDeviceRGBColorSpace, 0, 0);
  $.NSGraphicsContext.saveGraphicsState;
  $.NSGraphicsContext.setCurrentContext($.NSGraphicsContext.graphicsContextWithBitmapImageRep(rep));
  img.drawInRectFromRectOperationFraction($.NSMakeRect(0, 0, size, size), $.NSZeroRect, $.NSCompositingOperationSourceOver, 1.0);
  $.NSGraphicsContext.restoreGraphicsState;
  var png = rep.representationUsingTypeProperties($.NSBitmapImageFileTypePNG, $());
  return png.writeToFileAtomically(argv[1], true) ? 'ok' : 'fail';
}
"#;

/// Runs a helper and waits at most `limit`. Quick Look in particular can hang
/// forever on some files (source code, zips, apps); without a limit every worker
/// ends up stuck and no more thumbnails load.
fn run_with_timeout(mut cmd: Command, limit: Duration) -> bool {
    let Ok(mut child) = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else { return false };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(40)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn app_icon(path: &Path, out: &Path) -> bool {
    let mut cmd = Command::new("/usr/bin/osascript");
    cmd.args(["-l", "JavaScript", "-e", APP_ICON_JXA]).arg(path).arg(out).arg(THUMB_PX.to_string());
    run_with_timeout(cmd, Duration::from_secs(6)) && out.exists()
}

fn make_thumb(path: &Path, dir: &Path) -> Option<ColorImage> {
    let cached = dir.join(format!("{}.png", cache_key(path)));
    let is_app = path.extension().is_some_and(|e| e == "app");
    if !cached.exists() && is_app {
        let tmp = dir.join(format!("app-{}.png", cache_key(path)));
        if app_icon(path, &tmp) {
            std::fs::rename(&tmp, &cached).ok()?;
        }
    }
    if !cached.exists() && !is_app {
        // A folder per request, so a late or killed helper can never leave its
        // picture where another file's thumbnail is expected.
        let tmp = dir.join(format!("tmp-{}", cache_key(path)));
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::create_dir_all(&tmp);
        let mut cmd = Command::new("/usr/bin/qlmanage");
        cmd.args(["-t", "-s", &THUMB_PX.to_string(), "-o"]).arg(&tmp).arg(path);
        let ok = run_with_timeout(cmd, Duration::from_secs(8));
        // qlmanage writes "<file name>.png" into the output folder.
        let produced = ok
            .then(|| std::fs::read_dir(&tmp).ok()?.flatten().find(|e| e.path().extension().is_some_and(|x| x == "png")))
            .flatten();
        if let Some(produced) = produced {
            let _ = std::fs::rename(produced.path(), &cached);
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
    if !cached.exists() {
        return None;
    }
    let img = image::open(&cached).ok()?.into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

