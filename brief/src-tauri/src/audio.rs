//! Microphone capture via CPAL, output as 16 kHz mono f32 WAV (Whisper).
//!
//! The CPAL `Stream` is not `Send` on Linux and must not live in `AppState`.
//! Recording therefore runs in a dedicated thread; the stream stays there.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

/// Maximum buffer size in samples: ~4 hours at 48 kHz mono (~1.3 GB f32).
/// Prevents unbounded memory growth during very long recordings.
const MAX_BUFFER_SAMPLES: usize = 48_000 * 60 * 240;

/// Returns the names of all available input devices on the default CPAL host.
/// Used by the settings screen so users can choose a non-default microphone.
pub fn list_audio_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices
            .filter_map(|d| d.name().ok())
            .collect(),
        Err(e) => {
            log::error!("Failed to enumerate audio devices: {e}");
            vec![]
        }
    }
}

/// Holds one recording session: CPAL capture on a background thread, mono buffer, WAV output at 16 kHz.
pub struct AudioRecorder {
    pub session_id: String,
    pub meeting_type: String,
    buffer: Arc<Mutex<Vec<f32>>>,
    source_sample_rate: Option<u32>,
    stop_tx: Option<mpsc::Sender<()>>,
    join: Option<JoinHandle<()>>,
    /// RMS of the most-recently captured block, updated by the CPAL callback. Used for the UI level meter.
    pub last_rms: Arc<Mutex<f32>>,
}

impl AudioRecorder {
    /// Creates a recorder for `session_id` with the given `meeting_type` label (stored for future use).
    pub fn new(session_id: String, meeting_type: String) -> Self {
        AudioRecorder {
            session_id,
            meeting_type,
            buffer: Arc::new(Mutex::new(Vec::new())),
            source_sample_rate: None,
            stop_tx: None,
            join: None,
            last_rms: Arc::new(Mutex::new(0.0)),
        }
    }

    /// Opens the preferred input device (by name) or falls back to the system default when `device_name` is `None`
    /// or does not match any available device. Spawns the CPAL stream on a dedicated thread (Linux `Stream` is not
    /// `Send`) and fills the mono `f32` buffer until [`AudioRecorder::stop_and_save`] signals stop.
    pub fn start(&mut self) -> Result<(), String> {
        self.start_with_device(None)
    }

    /// Internal implementation used by both `start()` and the settings-driven code path.
    pub fn start_with_device(&mut self, device_name: Option<&str>) -> Result<(), String> {
        // Macro to avoid repeating the identical build_input_stream boilerplate for each SampleFormat.
        // Only the concrete sample type `$T` differs between arms; everything else is identical.
        // The callback also computes per-block RMS and stores it in `$rms` for the level meter.
        macro_rules! build_stream_for_format {
            ($device:expr, $config:expr, $buf:expr, $channels:expr, $err_fn:expr, $rms:expr, $T:ty) => {{
                let buf = Arc::clone(&$buf);
                let rms_cell = Arc::clone(&$rms);
                $device.build_input_stream(
                    $config,
                    move |data: &[$T], _: &_| {
                        push_mono_frames(data, $channels, &buf);
                        // Compute RMS of the incoming block for the level meter (O(block) not O(total)).
                        if !data.is_empty() {
                            let sum_sq: f32 = data.iter()
                                .map(|s| { let v: f32 = (*s).to_sample::<f32>(); v * v })
                                .sum();
                            let rms = (sum_sq / data.len() as f32).sqrt();
                            if let Ok(mut r) = rms_cell.lock() { *r = rms; }
                        }
                    },
                    $err_fn,
                    None,
                )
            }};
        }
        let host = cpal::default_host();
        // If a specific device name is requested, attempt to match it; fall back to default on miss.
        let device = if let Some(name) = device_name {
            host.input_devices()
                .ok()
                .and_then(|mut devs| devs.find(|d| d.name().ok().as_deref() == Some(name)))
                .or_else(|| host.default_input_device())
        } else {
            host.default_input_device()
        }
        .ok_or_else(|| "No microphone found".to_string())?;

        let device_display = device.name().unwrap_or_else(|_| "<unknown>".to_string());
        log::info!("Starting audio recording on device: {}", device_display);

        let supported = device.default_input_config().map_err(|e| e.to_string())?;

        let sample_rate = supported.sample_rate().0;
        self.source_sample_rate = Some(sample_rate);

        let channels = supported.channels() as usize;
        let stream_config: cpal::StreamConfig = supported.clone().into();
        let sample_format = supported.sample_format();
        let buffer = Arc::clone(&self.buffer);
        let last_rms = Arc::clone(&self.last_rms);

        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let join = std::thread::spawn(move || {
            let err_fn = |err: cpal::StreamError| log::error!("Audio stream error: {err}");

            let stream_result = match sample_format {
                cpal::SampleFormat::I8 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, i8),
                cpal::SampleFormat::I16 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, i16),
                cpal::SampleFormat::I32 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, i32),
                cpal::SampleFormat::I64 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, i64),
                cpal::SampleFormat::U8 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, u8),
                cpal::SampleFormat::U16 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, u16),
                cpal::SampleFormat::U32 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, u32),
                cpal::SampleFormat::U64 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, u64),
                cpal::SampleFormat::F32 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, f32),
                cpal::SampleFormat::F64 => build_stream_for_format!(device, &stream_config, buffer, channels, err_fn, last_rms, f64),
                f => {
                    log::error!("Unsupported audio sample format: {f}");
                    return;
                }
            };

            let stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to build audio stream: {e}");
                    return;
                }
            };

            if let Err(e) = stream.play() {
                log::error!("Failed to start audio stream: {e}");
                return;
            }

            let _ = stop_rx.recv();
            drop(stream);
        });

        self.stop_tx = Some(stop_tx);
        self.join = Some(join);
        Ok(())
    }

    /// Stops capture, joins the stream thread, resamples buffered audio to 16 kHz mono, and writes a float WAV at `output_path`.
    pub fn stop_and_save(&mut self, output_path: &PathBuf) -> Result<(), String> {
        log::info!("Stopping recording and saving WAV to {}", output_path.display());
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(j) = self.join.take() {
            j.join()
                .map_err(|_| "Recording thread could not be joined".to_string())?;
        }

        let source_rate = self
            .source_sample_rate
            .ok_or_else(|| "Internal error: source sample rate not set".to_string())?;

        let buf = self
            .buffer
            .lock()
            .map_err(|_| "Recording buffer not readable (Mutex poisoned)".to_string())?;

        let samples_16k = resample_to_16k(&buf, source_rate);

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = hound::WavWriter::create(output_path, spec).map_err(|e| e.to_string())?;

        for sample in samples_16k {
            writer.write_sample(sample).map_err(|e| e.to_string())?;
        }

        writer.finalize().map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn push_mono_frames<T>(input: &[T], channels: usize, buffer: &Arc<Mutex<Vec<f32>>>)
where
    T: Sample,
    f32: FromSample<T>,
{
    let ch = channels.max(1);
    if let Ok(mut buf) = buffer.lock() {
        // Drop incoming frames if the buffer has reached the safety cap.
        if buf.len() >= MAX_BUFFER_SAMPLES {
            return;
        }
        for frame in input.chunks(ch) {
            let n = frame.len() as f32;
            let mono = frame.iter().copied().map(f32::from_sample).sum::<f32>() / n;
            buf.push(mono);
        }
    }
}

fn resample_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if source_rate == 16000 {
        return samples.to_vec();
    }
    log::debug!("Resampling {} samples from {} Hz to 16000 Hz", samples.len(), source_rate);
    let ratio = 16000.0 / source_rate as f64;
    if samples.is_empty() {
        return Vec::new();
    }
    let new_len = ((samples.len() as f64) * ratio).ceil() as usize;
    (0..new_len)
        .map(|i| {
            let src_idx = ((i as f64) / ratio).floor() as usize;
            samples[src_idx.min(samples.len() - 1)]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn resample_to_16k_identity_when_already_16k() {
        let samples: Vec<f32> = (0..160).map(|i| i as f32 / 100.0).collect();
        let out = resample_to_16k(&samples, 16000);
        assert_eq!(out.len(), samples.len());
        assert_eq!(out, samples);
    }

    #[test]
    fn resample_to_16k_48khz_length_and_ratio() {
        // One second at 48 kHz → 48_000 input samples → ~16_000 output (16 kHz).
        let samples: Vec<f32> = vec![1.0; 48_000];
        let out = resample_to_16k(&samples, 48_000);
        assert_eq!(out.len(), 16_000);
        assert!(out.iter().all(|&s| (s - 1.0).abs() < f32::EPSILON));
    }

    #[test]
    fn resample_to_16k_empty_input() {
        let out = resample_to_16k(&[], 44100);
        assert!(out.is_empty());
    }

    #[test]
    fn resample_to_16k_single_sample() {
        let out = resample_to_16k(&[0.5], 48000);
        assert!(!out.is_empty());
        assert!((out[0] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn resample_to_16k_upsampling_8khz() {
        // 1 second at 8 kHz → 8_000 samples → ~16_000 output.
        let samples: Vec<f32> = vec![0.25; 8_000];
        let out = resample_to_16k(&samples, 8_000);
        assert_eq!(out.len(), 16_000);
    }

    #[test]
    fn push_mono_frames_stereo_averages_channels() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        // Stereo frame: left=0.0, right=1.0 → mono=0.5
        let input: Vec<f32> = vec![0.0, 1.0];
        push_mono_frames(&input, 2, &buffer);
        let buf = buffer.lock().unwrap();
        assert_eq!(buf.len(), 1);
        assert!((buf[0] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn push_mono_frames_mono_passthrough() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let input: Vec<f32> = vec![0.3, 0.7];
        push_mono_frames(&input, 1, &buffer);
        let buf = buffer.lock().unwrap();
        assert_eq!(buf.len(), 2);
        assert!((buf[0] - 0.3).abs() < f32::EPSILON);
        assert!((buf[1] - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn push_mono_frames_respects_buffer_cap() {
        let buffer = Arc::new(Mutex::new(Vec::with_capacity(0)));
        // Fill to exactly the cap.
        {
            let mut buf = buffer.lock().unwrap();
            buf.resize(MAX_BUFFER_SAMPLES, 0.0);
        }
        // Further samples should be dropped.
        let input: Vec<f32> = vec![1.0; 100];
        push_mono_frames(&input, 1, &buffer);
        let buf = buffer.lock().unwrap();
        assert_eq!(buf.len(), MAX_BUFFER_SAMPLES);
    }
}
