//! Page background: an optional image, ordered-dithered (the Aceternity
//! "dither shader" trick, CPU-side) and shown behind the whole window.
//!
//! GPUI has no public fragment-shader hook, so the pass runs once when the
//! setting changes: the chosen file is copied into `~/.orbit-pi/`, downscaled,
//! dithered cell-by-cell in **color** — each channel quantized with the Bayer
//! threshold, like the shader's `colorMode: "original"` — and cached as a
//! [`RenderImage`]. Painting it afterwards is a plain `img()` — no per-frame
//! work. The view adds the dim + bottom fade that drops it into the page.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use gpui::RenderImage;
use image::{Frame, RgbaImage};

/// Dither cell size, in source pixels — the shader's `gridSize`.
const GRID: u32 = 4;
/// Longest source edge kept; bigger only costs dither time.
const MAX_EDGE: u32 = 1600;
/// Brightness multiplier: the backdrop sits behind the whole UI, so it runs
/// dim — the chroma, not the brightness, is what carries it.
const DIM: f32 = 0.70;
/// Quantization levels per channel (3 → 27 colors, 4 → 64). Fewer levels
/// make the ordered pattern read more strongly.
const LEVELS: u32 = 4;
/// Contrast stretch applied before quantizing, so photos use the full ramp
/// instead of dithering everything to the middle.
const CONTRAST: f32 = 1.3;

/// Bayer 4×4 ordered-dither thresholds (0..15).
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

// ── paths & config ─────────────────────────────────────────────────────────

fn default_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".orbit-pi")
}

fn image_path(dir: &Path) -> PathBuf {
    dir.join("new-task-background.png")
}

fn config_path(dir: &Path) -> PathBuf {
    dir.join("background.json")
}

/// The picked file's display name, from `background.json`.
fn read_label(dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(config_path(dir)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("source")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

// ── setting changes ────────────────────────────────────────────────────────

/// Copy `source` into `dir` as the processed PNG, remembering its name.
/// Returns that name for the settings card.
pub fn choose(dir: &Path, source: &Path) -> Result<String, String> {
    let decoded = image::open(source).map_err(|err| format!("Could not read that image: {err}"))?;
    let fitted = fit(decoded);
    std::fs::create_dir_all(dir).map_err(|err| format!("Could not save the image: {err}"))?;
    fitted
        .into_rgba8()
        .save(image_path(dir))
        .map_err(|err| format!("Could not save the image: {err}"))?;
    let label = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string());
    let _ = std::fs::write(
        config_path(dir),
        serde_json::json!({ "source": label }).to_string(),
    );
    Ok(label)
}

/// Drop the background — the dot grid returns.
pub fn clear(dir: &Path) {
    let _ = std::fs::remove_file(image_path(dir));
    let _ = std::fs::remove_file(config_path(dir));
}

/// Downscale to [`MAX_EDGE`] so the dither pass stays cheap; smaller images
/// pass through untouched.
fn fit(image: image::DynamicImage) -> image::DynamicImage {
    if image.width() > MAX_EDGE || image.height() > MAX_EDGE {
        image.resize(MAX_EDGE, MAX_EDGE, image::imageops::FilterType::Triangle)
    } else {
        image
    }
}

// ── the backdrop ───────────────────────────────────────────────────────────

/// The configured setting's display name for the Settings card.
pub fn configured_label() -> Option<String> {
    read_label(&default_dir())
}

pub fn choose_file(source: &Path) -> Result<String, String> {
    choose(&default_dir(), source)
}

pub fn clear_all() {
    clear(&default_dir())
}

/// The dithered background, if one is configured. Decodes and dithers on
/// first call (or after a change), then serves the cached image.
pub fn background() -> Option<Arc<RenderImage>> {
    cached(&default_dir())
}

struct Cached {
    key: (PathBuf, u64, u64),
    image: Option<Arc<RenderImage>>,
}

static CACHE: RwLock<Option<Cached>> = RwLock::new(None);

/// Load + dither the stored PNG, memoized on `(path, mtime, len)` — choosing
/// and clearing change the file, so stale entries fall out on key mismatch
/// without explicit invalidation.
fn cached(dir: &Path) -> Option<Arc<RenderImage>> {
    let path = image_path(dir);
    let (modified, len) = std::fs::metadata(&path)
        .map(|meta| {
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_millis() as u64)
                .unwrap_or(0);
            (modified, meta.len())
        })
        .unwrap_or((0, 0));
    let key = (path.clone(), modified, len);

    let cache = CACHE.read().expect("dither cache");
    if let Some(cached) = cache.as_ref() {
        if cached.key == key {
            return cached.image.clone();
        }
    }
    drop(cache);

    let image = load(&path).map(|source| {
        let dithered = dither(&source, GRID);
        // GPUI's sprite atlas samples BGRA, not RGBA.
        let mut bgra = dithered;
        for pixel in bgra.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        Arc::new(RenderImage::new(vec![Frame::new(bgra)]))
    });
    *CACHE.write().expect("dither cache") = Some(Cached {
        key,
        image: image.clone(),
    });
    image
}

fn load(path: &Path) -> Option<RgbaImage> {
    image::open(path).ok().map(|image| image.into_rgba8())
}

/// Ordered-dither `source` in color, in `grid`-sized cells each sampled at
/// its centre. Every channel is dimmed, contrast-stretched, and quantized
/// with the cell's Bayer threshold — hue survives, the posterized ordered
/// pattern is what shows. Returns an RGBA buffer the size of the source.
fn dither(source: &RgbaImage, grid: u32) -> RgbaImage {
    let grid = grid.max(1);
    let (width, height) = source.dimensions();
    let mut out = RgbaImage::new(width, height);
    for cell_y in 0..height.div_ceil(grid) {
        for cell_x in 0..width.div_ceil(grid) {
            let sample_x = (cell_x * grid + grid / 2).min(width - 1);
            let sample_y = (cell_y * grid + grid / 2).min(height - 1);
            let sample = source.get_pixel(sample_x, sample_y).0;
            let threshold = (BAYER[cell_y as usize % 4][cell_x as usize % 4] as f32 + 0.5) / 16.;
            let color = [
                quantize(sample[0], threshold),
                quantize(sample[1], threshold),
                quantize(sample[2], threshold),
                sample[3],
            ];
            for y in cell_y * grid..(cell_y * grid + grid).min(height) {
                for x in cell_x * grid..(cell_x * grid + grid).min(width) {
                    out.put_pixel(x, y, image::Rgba(color));
                }
            }
        }
    }
    out
}

/// Dim, stretch, and quantize one channel using a Bayer threshold: the
/// fractional part of the scaled value decides whether it rounds up in this
/// cell, which is what turns a smooth ramp into an ordered pattern.
fn quantize(channel: u8, threshold: f32) -> u8 {
    let value = contrast((channel as f32 / 255. * DIM).clamp(0., 1.));
    let levels = (LEVELS - 1) as f32;
    let scaled = value * levels;
    let base = scaled.floor();
    let level = if scaled - base > threshold {
        base + 1.
    } else {
        base
    };
    ((level.clamp(0., levels) / levels) * 255.)
        .round()
        .clamp(0., 255.) as u8
}

fn contrast(value: f32) -> f32 {
    ((value - 0.5) * CONTRAST + 0.5).clamp(0., 1.)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, value: u8) -> RgbaImage {
        RgbaImage::from_pixel(width, height, image::Rgba([value, value, value, 255]))
    }

    #[test]
    fn bayer_is_a_permutation_of_zero_to_fifteen() {
        let mut seen: Vec<u8> = BAYER.iter().flatten().copied().collect();
        seen.sort_unstable();
        assert_eq!(seen, (0u8..16).collect::<Vec<_>>());
    }

    #[test]
    fn dither_keeps_size_and_quantizes_every_channel() {
        // 16×16 so every Bayer row/column is exercised (a shorter image can
        // miss the extremes of the matrix).
        let source = solid(16, 16, 128);
        let out = dither(&source, 4);
        assert_eq!(out.dimensions(), (16, 16));
        let levels: Vec<u8> = (0..LEVELS)
            .map(|level| (level * 255 / (LEVELS - 1)) as u8)
            .collect();
        for pixel in out.pixels() {
            for channel in &pixel.0[..3] {
                assert!(
                    levels.contains(channel),
                    "{channel} is not a quantized level"
                );
            }
        }
        // A mid-gray sits between two levels, so it must dither: more than
        // one level appears across the cells.
        let distinct: std::collections::HashSet<u8> =
            out.pixels().map(|pixel| pixel.0[0]).collect();
        assert!(distinct.len() > 1, "mid-gray should dither across levels");
    }

    #[test]
    fn color_survives_the_pass() {
        // The reference look keeps the photo's chroma — a red stays red
        // (dominant channel), rather than collapsing into two theme tones.
        let red = RgbaImage::from_pixel(8, 8, image::Rgba([210, 40, 30, 255]));
        let out = dither(&red, 4);
        assert!(out.pixels().all(|pixel| {
            let [r, g, b, _] = pixel.0;
            r > g && r > b
        }));
        // And a teal stays teal (the screenshot's hair color).
        let teal = RgbaImage::from_pixel(8, 8, image::Rgba([40, 190, 200, 255]));
        let out = dither(&teal, 4);
        assert!(out.pixels().all(|pixel| {
            let [r, g, b, _] = pixel.0;
            g > r && b > r
        }));
    }

    #[test]
    fn bright_input_stays_brighter_than_dark() {
        let bright = dither(&solid(16, 16, 255), 4);
        let dark = dither(&solid(16, 16, 0), 4);
        let mean = |image: &RgbaImage| {
            let sum: u32 = image.pixels().map(|pixel| pixel.0[0] as u32).sum();
            sum as f32 / (image.width() * image.height()) as f32
        };
        assert!(mean(&bright) > mean(&dark), "white must out-rank black");
        // Dimmed: even a full-white source averages below full white.
        assert!(mean(&bright) < 255., "dimming must hold white back");
    }

    #[test]
    fn grid_cells_are_uniform() {
        // Every grid-sized cell carries exactly one color.
        let source = solid(16, 16, 128);
        let out = dither(&source, 4);
        for cell_y in 0..4 {
            for cell_x in 0..4 {
                let first = out.get_pixel(cell_x * 4, cell_y * 4).0;
                for y in cell_y * 4..cell_y * 4 + 4 {
                    for x in cell_x * 4..cell_x * 4 + 4 {
                        assert_eq!(out.get_pixel(x, y).0, first, "cell ({cell_x},{cell_y})");
                    }
                }
            }
        }
    }

    #[test]
    fn choose_and_clear_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "orbit-dither-{}",
            std::process::id() as u64
                + std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64
        ));
        let source = dir.join("picked.png");
        std::fs::create_dir_all(&dir).unwrap();
        solid(4, 4, 90).save(&source).unwrap();

        let label = choose(&dir, &source).unwrap();
        assert_eq!(label, "picked.png");
        assert!(image_path(&dir).exists());
        assert_eq!(read_label(&dir).as_deref(), Some("picked.png"));

        clear(&dir);
        assert!(!image_path(&dir).exists());
        assert_eq!(read_label(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn choose_rejects_unreadable_files() {
        let dir =
            std::env::temp_dir().join(format!("orbit-dither-bad-{}", std::process::id() as u64));
        std::fs::create_dir_all(&dir).unwrap();
        let bogus = dir.join("not-an-image.txt");
        std::fs::write(&bogus, "hello").unwrap();
        assert!(choose(&dir, &bogus).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore] // manual: cargo test -p orbit-pi debug_render -- --ignored
    fn debug_render_to_tmp() {
        // Synthetic scene: a tonal ramp, color bands, and a bright disc —
        // enough to eyeball dimming, hue retention, and the dither grid.
        let mut source = RgbaImage::new(480, 270);
        for (x, y, pixel) in source.enumerate_pixels_mut() {
            let ramp = (x as f32 / 479. * 255.) as u8;
            let band = match y / 54 {
                0 => [ramp, ramp / 2, ramp / 3], // warm
                1 => [ramp / 3, ramp, ramp / 2], // green
                2 => [40, 190, 200],             // teal
                3 => [210, 60, 70],              // red
                _ => [ramp / 2, ramp / 3, ramp], // violet
            };
            *pixel = image::Rgba([band[0], band[1], band[2], 255]);
        }
        for (x, y, pixel) in source.enumerate_pixels_mut() {
            let dx = x as f32 - 360.;
            let dy = y as f32 - 90.;
            if dx * dx + dy * dy < 45. * 45. {
                *pixel = image::Rgba([240, 240, 240, 255]);
            }
        }
        let out = dither(&source, GRID);
        out.save("/tmp/dither-color.png").unwrap();
        println!("/tmp/dither-color.png dim={DIM} levels={LEVELS} grid={GRID}");
    }
}
