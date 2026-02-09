// audio.rs — System audio capture via WASAPI loopback (Windows)
//
// Captures what the speakers/headphones are playing using cpal's WASAPI backend.
// Audio is chunked into ~250ms segments, converted to PCM 16-bit @ 24kHz,
// and emitted as Tauri events for the UI audio meter and AI processing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use base64::Engine as _;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use tauri::Emitter;

// ─── Event Payloads ────────────────────────────────────────────────────────────

/// Payload for the `capture:audio-level` event.
/// The UI audio meter subscribes to this to show a VU-style bar.
#[derive(Clone, Serialize)]
pub struct AudioLevelPayload {
    /// RMS level normalized to 0.0–1.0
    pub level: f32,
    /// ISO-8601 timestamp of when the chunk was captured
    pub timestamp: String,
}

/// Payload for the `capture:audio-chunk` event.
/// Contains raw PCM bytes (base64-encoded) for AI processing.
#[derive(Clone, Serialize)]
pub struct AudioChunkPayload {
    /// Base64-encoded PCM 16-bit samples
    pub data: String,
    /// ISO-8601 timestamp
    pub timestamp: String,
    /// Sample rate of the PCM data (e.g. 24000)
    pub sample_rate: u32,
    /// Duration of this chunk in milliseconds
    pub duration_ms: u32,
}

// ─── AudioCapture ──────────────────────────────────────────────────────────────

/// Manages system audio capture via WASAPI loopback.
///
/// # Usage
/// ```ignore
/// let capture = AudioCapture::new(24000, 250);
/// capture.toggle();                       // start
/// capture.start_loop(app_handle.clone());  // runs on a background thread
/// capture.toggle();                       // stop
/// ```
pub struct AudioCapture {
    /// Shared flag — `true` while capturing, `false` to stop.
    is_capturing: Arc<AtomicBool>,
    /// Target output sample rate (Hz). Default: 24 000.
    sample_rate: u32,
    /// How many milliseconds of audio per chunk. Default: 250.
    chunk_ms: u32,
}

impl AudioCapture {
    /// Create a new `AudioCapture` with the given sample rate and chunk size.
    ///
    /// * `sample_rate` — target PCM sample rate in Hz (e.g. 24000)
    /// * `chunk_ms`    — how often to emit a chunk, in milliseconds (e.g. 250)
    pub fn new(sample_rate: u32, chunk_ms: u32) -> Self {
        Self {
            is_capturing: Arc::new(AtomicBool::new(false)),
            sample_rate,
            chunk_ms,
        }
    }

    /// Flip the capturing flag. Returns `true` if capturing is now **on**.
    pub fn toggle(&self) -> bool {
        // fetch_xor flips the bool and returns the *previous* value
        let was_capturing = self.is_capturing.fetch_xor(true, Ordering::SeqCst);
        let now_capturing = !was_capturing;
        log::info!("Audio capture toggled → {}", if now_capturing { "ON" } else { "OFF" });
        now_capturing
    }

    /// Returns `true` if audio capture is currently active.
    pub fn is_capturing(&self) -> bool {
        self.is_capturing.load(Ordering::SeqCst)
    }

    /// Start the audio capture loop on a **background thread**.
    ///
    /// This function spawns a `std::thread` (not a tokio task) because cpal
    /// streams are `!Send` on some backends. The thread will keep running
    /// until `is_capturing` is set to `false` (via [`toggle`]).
    ///
    /// Events emitted:
    /// - `capture:audio-level`  — every `chunk_ms` with the RMS level
    /// - `capture:audio-chunk`  — every `chunk_ms` with base64-encoded PCM data
    pub fn start_loop(&self, app_handle: tauri::AppHandle) {
        let is_capturing = Arc::clone(&self.is_capturing);
        let sample_rate = self.sample_rate;
        let chunk_ms = self.chunk_ms;

        std::thread::spawn(move || {
            if let Err(e) = run_capture_loop(is_capturing, app_handle, sample_rate, chunk_ms) {
                log::error!("Audio capture loop failed: {e}");
            }
        });
    }
}

// ─── Internal capture loop ─────────────────────────────────────────────────────

/// The actual capture loop. Runs on a dedicated OS thread.
fn run_capture_loop(
    is_capturing: Arc<AtomicBool>,
    app_handle: tauri::AppHandle,
    target_rate: u32,
    chunk_ms: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Pick the default WASAPI host & output device.
    //    On Windows, building an *input* stream on an *output* device gives us
    //    loopback capture (i.e. we hear what the speakers play).
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("No default output device found")?;

    log::info!("Audio capture device: {:?}", device.name()?);

    // 2. Get the device's default output config so we know its native format.
    let supported_config = device.default_output_config()?;
    let device_sample_rate = supported_config.sample_rate().0;
    let device_channels = supported_config.channels() as usize;
    let sample_format = supported_config.sample_format();

    log::info!(
        "Device config: {}Hz, {} ch, {:?}",
        device_sample_rate,
        device_channels,
        sample_format
    );

    // 3. Compute how many *device-rate* samples we need per chunk.
    //    We'll accumulate samples in a buffer, then resample + emit.
    let device_samples_per_chunk =
        (device_sample_rate as usize * chunk_ms as usize) / 1000 * device_channels;

    // Shared buffer: the cpal callback pushes samples here, the drain loop reads them.
    let buffer: Arc<std::sync::Mutex<Vec<f32>>> =
        Arc::new(std::sync::Mutex::new(Vec::with_capacity(device_samples_per_chunk * 2)));

    let buffer_writer = Arc::clone(&buffer);

    // 4. Build the stream config from the device's supported config.
    let stream_config: cpal::StreamConfig = supported_config.into();

    // 5. Build the input stream (loopback on Windows WASAPI).
    //    We convert every sample format to f32 for uniform processing.
    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = buffer_writer.lock() {
                    buf.extend_from_slice(data);
                }
            },
            |err| log::error!("Audio stream error: {err}"),
            None, // no timeout
        )?,
        cpal::SampleFormat::I16 => {
            let buf_w = Arc::clone(&buffer);
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    // Convert i16 → f32 (range -1.0..1.0)
                    let floats: Vec<f32> = data
                        .iter()
                        .map(|&s| s as f32 / i16::MAX as f32)
                        .collect();
                    if let Ok(mut buf) = buf_w.lock() {
                        buf.extend_from_slice(&floats);
                    }
                },
                |err| log::error!("Audio stream error: {err}"),
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let buf_w = Arc::clone(&buffer);
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    // Convert u16 → f32 (range -1.0..1.0)
                    let floats: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                        .collect();
                    if let Ok(mut buf) = buf_w.lock() {
                        buf.extend_from_slice(&floats);
                    }
                },
                |err| log::error!("Audio stream error: {err}"),
                None,
            )?
        }
        other => return Err(format!("Unsupported sample format: {other:?}").into()),
    };

    // 6. Start the stream — cpal begins delivering audio to our callback.
    stream.play()?;
    log::info!("Audio capture stream started");

    // 7. Drain loop: every `chunk_ms` we pull accumulated samples,
    //    down-mix to mono, resample to `target_rate`, quantise to i16,
    //    compute RMS, and emit Tauri events.
    let chunk_duration = std::time::Duration::from_millis(chunk_ms as u64);

    while is_capturing.load(Ordering::SeqCst) {
        std::thread::sleep(chunk_duration);

        // Pull all accumulated samples out of the shared buffer.
        let raw_samples: Vec<f32> = {
            let mut buf = buffer.lock().unwrap();
            buf.drain(..).collect()
        };

        if raw_samples.is_empty() {
            continue;
        }

        // a) Down-mix to mono by averaging channels.
        let mono = downmix_to_mono(&raw_samples, device_channels);

        // b) Resample from device rate → target rate (simple linear interpolation).
        let resampled = resample(&mono, device_sample_rate, target_rate);

        // c) Convert f32 → i16 PCM samples.
        let pcm_i16: Vec<i16> = resampled
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();

        // d) Compute RMS level for the UI meter.
        let rms = compute_rms(&pcm_i16);

        // e) Get a timestamp for both events.
        let timestamp = now_iso8601();

        // f) Emit `capture:audio-level`.
        let level_payload = AudioLevelPayload {
            level: rms,
            timestamp: timestamp.clone(),
        };
        if let Err(e) = app_handle.emit("capture:audio-level", &level_payload) {
            log::debug!("Failed to emit audio-level: {e}");
        }

        // g) Encode PCM bytes as base64 and emit `capture:audio-chunk`.
        let pcm_bytes = pcm_i16_to_bytes(&pcm_i16);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&pcm_bytes);

        let chunk_payload = AudioChunkPayload {
            data: b64,
            timestamp,
            sample_rate: target_rate,
            duration_ms: chunk_ms,
        };
        if let Err(e) = app_handle.emit("capture:audio-chunk", &chunk_payload) {
            log::debug!("Failed to emit audio-chunk: {e}");
        }

        log::debug!("Emitted audio chunk: {} samples, RMS={:.4}", pcm_i16.len(), rms);
    }

    // 8. Capture was toggled off — the stream is dropped here automatically.
    log::info!("Audio capture loop stopped");
    Ok(())
}

// ─── DSP helpers ───────────────────────────────────────────────────────────────

/// Down-mix interleaved multi-channel audio to mono by averaging channels.
fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Resample audio using simple linear interpolation.
/// Good enough for speech/AI; not audiophile-grade.
fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return input.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = ((input.len() as f64) / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;

        let sample = if idx + 1 < input.len() {
            // Linear interpolation between two neighbouring samples
            input[idx] as f64 * (1.0 - frac) + input[idx + 1] as f64 * frac
        } else {
            input.get(idx).copied().unwrap_or(0.0) as f64
        };

        output.push(sample as f32);
    }

    output
}

/// Compute the RMS (Root Mean Square) of PCM i16 samples.
/// Returns a value in the range 0.0–1.0.
pub fn compute_rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum / samples.len() as f64).sqrt() as f32 / i16::MAX as f32
}

/// Convert a slice of i16 samples to raw little-endian bytes.
fn pcm_i16_to_bytes(samples: &[i16]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect()
}

/// Returns the current time as an ISO-8601 string (UTC, millisecond precision).
/// Uses `std::time::SystemTime` to avoid pulling in the `chrono` crate.
fn now_iso8601() -> String {
    // Seconds since UNIX epoch
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    // Convert to a rough UTC date-time (no leap seconds, good enough for events)
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Convert days since epoch to year-month-day (simplified Gregorian)
    let (year, month, day) = epoch_days_to_ymd(days);

    format!(
        "{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z"
    )
}

/// Convert days since the UNIX epoch (1970-01-01) to (year, month, day).
fn epoch_days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from Howard Hinnant's date library (civil_from_days)
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u64, m, d)
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_of_silence_is_zero() {
        let silence = vec![0i16; 1000];
        assert_eq!(compute_rms(&silence), 0.0);
    }

    #[test]
    fn rms_of_max_volume_is_close_to_one() {
        // A constant signal at i16::MAX should give RMS ≈ 1.0
        let loud = vec![i16::MAX; 1000];
        let rms = compute_rms(&loud);
        assert!(
            (rms - 1.0).abs() < 0.001,
            "Expected RMS ≈ 1.0, got {rms}"
        );
    }

    #[test]
    fn rms_of_known_signal() {
        // A square wave toggling between +16384 and -16384.
        // RMS of a square wave = amplitude.
        // Normalized: 16384 / 32767 ≈ 0.50002
        let amplitude: i16 = 16384;
        let signal: Vec<i16> = (0..1000)
            .map(|i| if i % 2 == 0 { amplitude } else { -amplitude })
            .collect();
        let rms = compute_rms(&signal);
        let expected = amplitude as f32 / i16::MAX as f32;
        assert!(
            (rms - expected).abs() < 0.001,
            "Expected RMS ≈ {expected}, got {rms}"
        );
    }

    #[test]
    fn rms_of_empty_is_zero() {
        assert_eq!(compute_rms(&[]), 0.0);
    }

    #[test]
    fn downmix_stereo_to_mono() {
        // Stereo: L=1.0, R=-1.0 → mono average = 0.0
        let stereo = vec![1.0f32, -1.0, 0.5, 0.5];
        let mono = downmix_to_mono(&stereo, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.0).abs() < f32::EPSILON);
        assert!((mono[1] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn resample_same_rate_is_identity() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let output = resample(&input, 48000, 48000);
        assert_eq!(input, output);
    }

    #[test]
    fn resample_halves_length_when_doubling_ratio() {
        // 48kHz → 24kHz should roughly halve the number of samples
        let input: Vec<f32> = (0..480).map(|i| (i as f32) / 480.0).collect();
        let output = resample(&input, 48000, 24000);
        // Allow ±1 sample for rounding
        assert!(
            (output.len() as i32 - 240).abs() <= 1,
            "Expected ~240 samples, got {}",
            output.len()
        );
    }

    #[test]
    fn pcm_bytes_roundtrip() {
        let samples: Vec<i16> = vec![0, 1, -1, i16::MAX, i16::MIN];
        let bytes = pcm_i16_to_bytes(&samples);
        assert_eq!(bytes.len(), samples.len() * 2);

        // Decode back
        let decoded: Vec<i16> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(samples, decoded);
    }

    #[test]
    fn iso8601_format_looks_valid() {
        let ts = now_iso8601();
        // Should match YYYY-MM-DDTHH:MM:SS.mmmZ
        assert!(ts.ends_with('Z'), "Timestamp should end with Z: {ts}");
        assert_eq!(ts.len(), 24, "Expected 24 chars, got {}: {ts}", ts.len());
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
    }
}
