// screen.rs — Captures the primary monitor, downscales, JPEG-encodes,
// base64-encodes, and emits Tauri events. Includes perceptual-hash
// frame diffing so unchanged screens are skipped.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::codecs::jpeg::JpegEncoder;
use image::{imageops, DynamicImage, GenericImageView, GrayImage};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

// ── Event payload ───────────────────────────────────────────────────────
/// The JSON payload emitted on every new frame via the `capture:frame` event.
#[derive(Clone, Serialize)]
pub struct FramePayload {
    /// Base64-encoded JPEG image data
    pub data: String,
    /// ISO-8601 timestamp of when the frame was captured
    pub timestamp: String,
    /// Width of the (possibly downscaled) image
    pub width: u32,
    /// Height of the (possibly downscaled) image
    pub height: u32,
    /// Hamming distance percentage between this frame and the previous one.
    /// 0.0 means identical, 100.0 means completely different.
    pub diff_pct: f64,
}

// ── ScreenCapture ───────────────────────────────────────────────────────
/// Holds capture configuration and runtime state.
///
/// ## Ownership & thread-safety
/// * `is_capturing` is an `Arc<AtomicBool>` — a thread-safe boolean that
///   can be shared across threads without a lock.
/// * `last_hash` uses a `Mutex` because we only need interior mutability
///   and the critical section is tiny.
/// * The struct itself is wrapped in `Arc` when stored in Tauri managed state
///   so multiple commands can reference it.
pub struct ScreenCapture {
    /// Whether the capture loop is currently running.
    is_capturing: Arc<AtomicBool>,
    /// Milliseconds between successive screen captures.
    interval_ms: u64,
    /// Maximum width in pixels; images wider than this are downscaled.
    max_width: u32,
    /// JPEG compression quality (1–100).
    jpeg_quality: u8,
    /// Perceptual hash of the most recently emitted frame, used for diffing.
    last_hash: Mutex<u64>,
}

impl ScreenCapture {
    /// Create a new `ScreenCapture` with the given settings.
    ///
    /// Typical defaults: `interval_ms = 2000`, `max_width = 1024`, `jpeg_quality = 75`.
    pub fn new(interval_ms: u64, max_width: u32, jpeg_quality: u8) -> Self {
        Self {
            is_capturing: Arc::new(AtomicBool::new(false)),
            interval_ms,
            max_width,
            jpeg_quality,
            last_hash: Mutex::new(0),
        }
    }

    /// Flip the capturing flag on/off. Returns the **new** state.
    ///
    /// `Ordering::SeqCst` (sequentially consistent) is the strongest memory
    /// ordering — fine for a toggle that happens infrequently.
    pub fn toggle(&self) -> bool {
        let was = self.is_capturing.fetch_xor(true, Ordering::SeqCst);
        // fetch_xor returns the *previous* value; XOR with true flips the bit.
        !was
    }

    /// Check whether the capture loop is active.
    #[allow(dead_code)]
    pub fn is_capturing(&self) -> bool {
        self.is_capturing.load(Ordering::SeqCst)
    }

    /// Start the capture loop on a background tokio task.
    ///
    /// The loop keeps running while `is_capturing` is true.
    /// Each iteration:
    ///   1. Grabs a screenshot via `xcap`
    ///   2. Computes a perceptual hash and skips if similar to the last frame
    ///   3. Downscales, JPEG-encodes, base64-encodes
    ///   4. Emits a `capture:frame` event on the Tauri app handle
    ///
    /// ## Why `&self` + clones?
    /// Tauri commands receive shared references, and `tokio::spawn` requires
    /// `'static` data.  We clone the `Arc`s / values we need so the spawned
    /// future owns them independently of `self`.
    pub async fn start_loop(
        &self,
        app_handle: AppHandle,
        stream_manager: Option<Arc<crate::stream_manager::StreamManager>>,
    ) {
        // Clone the pieces we need so the spawned task owns them.
        let flag = Arc::clone(&self.is_capturing);
        let interval = self.interval_ms;
        let max_w = self.max_width;
        let quality = self.jpeg_quality;

        // We need the Mutex to travel into the spawned task. Because
        // `Mutex<u64>` isn't Clone, we wrap access through a shared Arc.
        // However, `self.last_hash` is already inside `ScreenCapture` which
        // is stored in an Arc in Tauri state, so we use a separate Arc<Mutex>
        // here that we initialise from the current value.
        let prev_hash = {
            let h = self.last_hash.lock().unwrap();
            *h
        };
        let last_hash = Arc::new(Mutex::new(prev_hash));
        let last_hash_self = Arc::clone(&last_hash);

        tokio::spawn(async move {
            log::info!("Screen capture loop started (interval={}ms)", interval);

            while flag.load(Ordering::SeqCst) {
                match capture_frame(max_w, quality, &last_hash) {
                    Ok(Some(payload)) => {
                        log::debug!(
                            "Emitting capture:frame ({}x{}, diff={:.1}%)",
                            payload.width,
                            payload.height,
                            payload.diff_pct
                        );
                        // Send frame to AI pipeline if configured
                        if let Some(ref sm) = stream_manager {
                            sm.analyze_frame(payload.data.clone(), app_handle.clone());
                        }

                        if let Err(e) = app_handle.emit("capture:frame", &payload) {
                            log::error!("Failed to emit capture:frame: {}", e);
                        }
                    }
                    Ok(None) => {
                        log::debug!("Frame skipped (similar to previous)");
                    }
                    Err(e) => {
                        log::error!("Capture error: {}", e);
                    }
                }

                tokio::time::sleep(std::time::Duration::from_millis(interval)).await;
            }

            log::info!("Screen capture loop stopped");
        });

        // Sync back the hash so that if the loop is restarted, we keep the
        // last known hash for diffing continuity.
        // Note: This write happens after spawn returns (immediately), not
        // after the loop finishes, so it seeds the spawned task with the
        // current value — the spawned task maintains its own copy via
        // `last_hash` Arc.
        let final_h = last_hash_self.lock().unwrap();
        let mut self_h = self.last_hash.lock().unwrap();
        *self_h = *final_h;
    }
}

// ── Internal helpers ────────────────────────────────────────────────────

/// Grab a screenshot, diff it, and return the encoded payload (or None if
/// the frame is too similar to the previous one).
fn capture_frame(
    max_width: u32,
    jpeg_quality: u8,
    last_hash: &Arc<Mutex<u64>>,
) -> Result<Option<FramePayload>, String> {
    // 1. Capture the primary monitor
    let monitors = xcap::Monitor::all().map_err(|e| format!("enumerate monitors: {e}"))?;
    let monitor = monitors
        .into_iter()
        .find(|m| m.is_primary())
        .or_else(|| xcap::Monitor::all().ok()?.into_iter().next())
        .ok_or_else(|| "no monitors found".to_string())?;

    let raw = monitor
        .capture_image()
        .map_err(|e| format!("capture_image: {e}"))?;

    // `xcap` returns an `image::RgbaImage`. Wrap it in DynamicImage for
    // convenient manipulation.
    let img = DynamicImage::ImageRgba8(raw);

    // 2. Compute perceptual hash and diff
    let current_hash = compute_average_hash(&img);
    let distance = {
        let mut prev = last_hash.lock().unwrap();
        let d = hamming_distance(current_hash, *prev);
        *prev = current_hash;
        d
    };

    // Each bit of the 64-bit hash represents one 8×8 cell.
    // distance / 64.0 * 100.0 gives a percentage.
    let diff_pct = (distance as f64 / 64.0) * 100.0;

    // Skip if fewer than 5 bits differ (< ~7.8 % change)
    if distance < 5 {
        return Ok(None);
    }

    // 3. Downscale if wider than max_width
    let img = if img.width() > max_width {
        let ratio = max_width as f64 / img.width() as f64;
        let new_h = (img.height() as f64 * ratio).round() as u32;
        img.resize_exact(max_width, new_h, imageops::FilterType::Triangle)
    } else {
        img
    };

    let (w, h) = img.dimensions();

    // 4. JPEG encode
    let mut jpeg_buf: Vec<u8> = Vec::new();
    {
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, jpeg_quality);
        // `write_image` takes raw pixel bytes, dimensions, and colour type.
        encoder
            .encode(img.to_rgb8().as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .map_err(|e| format!("jpeg encode: {e}"))?;
    }

    // 5. Base64 encode
    let b64 = BASE64.encode(&jpeg_buf);

    // 6. Build timestamp
    let ts = chrono_now_iso();

    Ok(Some(FramePayload {
        data: b64,
        timestamp: ts,
        width: w,
        height: h,
        diff_pct,
    }))
}

/// Compute a 64-bit average hash (aHash) for perceptual image comparison.
///
/// Algorithm:
///   1. Downscale to 8×8 pixels
///   2. Convert to grayscale
///   3. Compute the mean brightness
///   4. Set each bit to 1 if that pixel is brighter than the mean, else 0
///
/// Two images with a small hamming distance between their hashes look similar.
pub fn compute_average_hash(img: &DynamicImage) -> u64 {
    // Shrink to 8×8 — this blurs away fine detail, keeping only structure.
    let small = img.resize_exact(8, 8, imageops::FilterType::Triangle);
    let gray: GrayImage = small.to_luma8();

    // Mean brightness (sum all pixel values, divide by 64).
    let sum: u64 = gray.pixels().map(|p| p.0[0] as u64).sum();
    let mean = sum / 64;

    // Build the 64-bit hash: one bit per pixel.
    let mut hash: u64 = 0;
    for (i, p) in gray.pixels().enumerate() {
        if (p.0[0] as u64) > mean {
            hash |= 1u64 << i;
        }
    }
    hash
}

/// Count how many bits differ between two hashes (XOR then popcount).
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Return the current UTC time as an ISO-8601 string.
/// Uses `std::time::SystemTime` to avoid adding a chrono dependency.
fn chrono_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    // Manual conversion — good enough for logging purposes.
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let mins = (time_secs % 3600) / 60;
    let s = time_secs % 60;
    let millis = dur.subsec_millis();

    // Days since Unix epoch → calendar date (simplified leap-year calc)
    let (year, month, day) = epoch_days_to_ymd(days as i64);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hours, mins, s, millis
    )
}

/// Convert days since Unix epoch (1970-01-01) to (year, month, day).
pub fn epoch_days_to_ymd(mut days: i64) -> (i64, u32, u32) {
    // Shift epoch from 1970-01-01 to 0000-03-01 for easier leap-year math.
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ── Tests ───────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbaImage};

    /// Helper: create a solid-colour RGBA image.
    fn solid_image(r: u8, g: u8, b: u8, w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([r, g, b, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    /// Helper: create a half-and-half image (left=black, right=white).
    fn split_image(w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for (x, _y, pixel) in img.enumerate_pixels_mut() {
            if x < w / 2 {
                *pixel = image::Rgba([0, 0, 0, 255]);
            } else {
                *pixel = image::Rgba([255, 255, 255, 255]);
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn hash_is_consistent_for_same_image() {
        let img = solid_image(100, 150, 200, 64, 64);
        let h1 = compute_average_hash(&img);
        let h2 = compute_average_hash(&img);
        assert_eq!(h1, h2, "same image must produce the same hash");
    }

    #[test]
    fn different_images_produce_different_hashes() {
        let white = solid_image(255, 255, 255, 64, 64);
        let black = solid_image(0, 0, 0, 64, 64);
        let split = split_image(64, 64);

        let h_white = compute_average_hash(&white);
        let h_black = compute_average_hash(&black);
        let h_split = compute_average_hash(&split);

        // A split image should differ from both solid images.
        assert_ne!(h_split, h_white, "split vs white should differ");
        assert_ne!(h_split, h_black, "split vs black should differ");
    }

    #[test]
    fn frame_diff_identifies_similar_frames() {
        let img_a = solid_image(128, 128, 128, 64, 64);
        // Nearly identical — just a tiny brightness shift.
        let img_b = solid_image(130, 130, 130, 64, 64);

        let h_a = compute_average_hash(&img_a);
        let h_b = compute_average_hash(&img_b);

        let dist = hamming_distance(h_a, h_b);
        assert!(
            dist < 5,
            "similar images should have hamming distance < 5 (got {})",
            dist
        );
    }

    #[test]
    fn frame_diff_detects_large_changes() {
        let white = solid_image(255, 255, 255, 64, 64);
        let split = split_image(64, 64);

        let h_w = compute_average_hash(&white);
        let h_s = compute_average_hash(&split);

        let dist = hamming_distance(h_w, h_s);
        assert!(
            dist >= 5,
            "visually different images should have hamming distance >= 5 (got {})",
            dist
        );
    }

    #[test]
    fn hamming_distance_basics() {
        assert_eq!(hamming_distance(0, 0), 0);
        assert_eq!(hamming_distance(0xFF, 0x00), 8);
        assert_eq!(hamming_distance(u64::MAX, 0), 64);
    }
}
