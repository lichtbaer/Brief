//! Microphone capture via CPAL, output as 16 kHz mono f32 WAV (Whisper).
//!
//! The CPAL `Stream` is not `Send` on Linux and must not live in `AppState`.
//! Recording therefore runs in a dedicated thread; the stream stays there.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

pub struct AudioRecorder {
    pub session_id: String,
    pub meeting_type: String,
    buffer: Arc<Mutex<Vec<f32>>>,
    source_sample_rate: Option<u32>,
    stop_tx: Option<mpsc::Sender<()>>,
    join: Option<JoinHandle<()>>,
}

impl AudioRecorder {
    pub fn new(session_id: String, meeting_type: String) -> Self {
        AudioRecorder {
            session_id,
            meeting_type,
            buffer: Arc::new(Mutex::new(Vec::new())),
            source_sample_rate: None,
            stop_tx: None,
            join: None,
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "Kein Mikrofon gefunden".to_string())?;

        let supported = device
            .default_input_config()
            .map_err(|e| e.to_string())?;

        let sample_rate = supported.sample_rate().0;
        self.source_sample_rate = Some(sample_rate);

        let channels = supported.channels() as usize;
        let stream_config: cpal::StreamConfig = supported.clone().into();
        let sample_format = supported.sample_format();
        let buffer = Arc::clone(&self.buffer);

        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let join = std::thread::spawn(move || {
            let err_fn = |err: cpal::StreamError| eprintln!("Audio-Fehler: {err}");

            let stream = match sample_format {
                cpal::SampleFormat::I8 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[i8], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::I16 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::I32 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[i32], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::I64 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[i64], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::U8 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[u8], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::U16 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::U32 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[u32], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::U64 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[u64], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::F32 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                cpal::SampleFormat::F64 => match {
                    let buf = Arc::clone(&buffer);
                    device.build_input_stream(
                    &stream_config,
                    move |data: &[f64], _: &_| {
                        push_mono_frames(data, channels, &buf);
                    },
                    err_fn,
                    None,
                    )
                } {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Stream konnte nicht erstellt werden: {e}");
                        return;
                    }
                },
                f => {
                    eprintln!("Nicht unterstütztes Audio-Sample-Format: {f}");
                    return;
                }
            };

            if let Err(e) = stream.play() {
                eprintln!("Stream konnte nicht gestartet werden: {e}");
                return;
            }

            let _ = stop_rx.recv();
            drop(stream);
        });

        self.stop_tx = Some(stop_tx);
        self.join = Some(join);
        Ok(())
    }

    pub fn stop_and_save(&mut self, output_path: &PathBuf) -> Result<(), String> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(j) = self.join.take() {
            j.join()
                .map_err(|_| "Aufnahme-Thread konnte nicht beendet werden".to_string())?;
        }

        let source_rate = self
            .source_sample_rate
            .ok_or_else(|| "Interner Fehler: keine Sample-Rate gesetzt".to_string())?;

        let buf = self
            .buffer
            .lock()
            .map_err(|_| "Aufnahme-Puffer nicht lesbar (Mutex)".to_string())?;

        let samples_16k = resample_to_16k(&buf, source_rate);

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer =
            hound::WavWriter::create(output_path, spec).map_err(|e| e.to_string())?;

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
        for frame in input.chunks(ch) {
            let n = frame.len() as f32;
            let mono = frame
                .iter()
                .copied()
                .map(f32::from_sample)
                .sum::<f32>()
                / n;
            buf.push(mono);
        }
    }
}

fn resample_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if source_rate == 16000 {
        return samples.to_vec();
    }
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
