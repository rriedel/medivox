//! Nimmt Audio zwischen start() und stop() auf (.NET-Pendant: Recorder.cs).
//! Kein VAD -- Start/Stop ist explizit.
//!
//! cpal liefert unter Windows das native WASAPI-Shared-Mode-Mixformat (typisch f32,
//! 48 kHz, stereo). Der Downmix auf Mono passiert direkt im Capture-Callback (haelt den
//! Puffer klein), das Resampling auf 16 kHz einmalig beim Stop -- Ersatz fuer den
//! MediaFoundationResampler der .NET-Fassung.

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use crate::config::SAMPLE_RATE;

type Buffer = Arc<Mutex<Vec<f32>>>;

#[derive(Default)]
pub struct Recorder {
    stream: Option<cpal::Stream>,
    buffer: Buffer,
    source_rate: u32,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("Kein Standard-Aufnahmegeraet gefunden"))?;
        let supported = device
            .default_input_config()
            .context("Aufnahmeformat des Geraets konnte nicht ermittelt werden")?;

        let channels = supported.channels() as usize;
        self.source_rate = supported.sample_rate();

        self.buffer = Arc::new(Mutex::new(Vec::new()));
        let config: cpal::StreamConfig = supported.config();
        let err_fn = |err| tracing::error!("Fehler im Audio-Stream: {err}");

        let stream = match supported.sample_format() {
            SampleFormat::F32 => {
                let buffer = Arc::clone(&self.buffer);
                device.build_input_stream(
                    config,
                    move |data: &[f32], _: &_| downmix(data, channels, &buffer),
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let buffer = Arc::clone(&self.buffer);
                device.build_input_stream(
                    config,
                    move |data: &[i16], _: &_| {
                        let floats: Vec<f32> =
                            data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                        downmix(&floats, channels, &buffer);
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let buffer = Arc::clone(&self.buffer);
                device.build_input_stream(
                    config,
                    move |data: &[u16], _: &_| {
                        let floats: Vec<f32> = data
                            .iter()
                            .map(|s| (*s as f32 - 32768.0) / 32768.0)
                            .collect();
                        downmix(&floats, channels, &buffer);
                    },
                    err_fn,
                    None,
                )
            }
            other => return Err(anyhow!("Nicht unterstuetztes Sample-Format: {other:?}")),
        }
        .context("Audio-Stream konnte nicht geoeffnet werden")?;

        stream.play().context("Aufnahme konnte nicht starten")?;
        self.stream = Some(stream);
        tracing::debug!(
            "Aufnahme gestartet ({} Hz, {} Kanaele)",
            self.source_rate,
            channels
        );
        Ok(())
    }

    /// Stoppt die Aufnahme und liefert die Samples als 16 kHz mono float32.
    pub fn stop(&mut self) -> Result<Vec<f32>> {
        // Drop stoppt den Stream und beendet die Callbacks.
        self.stream.take();

        let samples = {
            let mut guard = self.buffer.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        if samples.is_empty() {
            return Ok(Vec::new());
        }
        if self.source_rate == SAMPLE_RATE {
            return Ok(samples);
        }

        let resampled = resample(&samples, self.source_rate, SAMPLE_RATE)?;
        tracing::debug!(
            "Aufnahme gestoppt: {} Frames @ {} Hz -> {} Frames @ {} Hz",
            samples.len(),
            self.source_rate,
            resampled.len(),
            SAMPLE_RATE
        );
        Ok(resampled)
    }
}

fn downmix(data: &[f32], channels: usize, buffer: &Buffer) {
    let Ok(mut guard) = buffer.lock() else {
        return;
    };
    if channels <= 1 {
        guard.extend_from_slice(data);
        return;
    }
    for frame in data.chunks_exact(channels) {
        guard.push(frame.iter().sum::<f32>() / channels as f32);
    }
}

/// Mono-Resampling per Sinc-Interpolation. Der Resampler laeuft mit fester Blockgroesse;
/// der letzte, unvollstaendige Block wird ueber Indexing::partial_len abgeschlossen. Die
/// Eigenverzoegerung des Filters (output_delay) wird vorne abgeschnitten, sonst begaenne
/// die Aufnahme mit ein paar Millisekunden Stille.
fn resample(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    const CHUNK: usize = 1024;

    let ratio = to as f64 / from as f64;
    let params = SincInterpolationParameters::new(256, WindowFunction::BlackmanHarris2)
        .oversampling_factor(256)
        .interpolation(SincInterpolationType::Linear);
    let mut resampler = Async::<f32>::new_sinc(ratio, 1.1, &params, CHUNK, 1, FixedAsync::Input)
        .context("Resampler konnte nicht erzeugt werden")?;

    let frames_in = input.len();
    let expected = (frames_in as f64 * ratio).ceil() as usize;
    let delay = resampler.output_delay();

    let mut output = vec![0.0f32; expected + delay + CHUNK];
    let capacity = output.len();
    let adapter_in =
        InterleavedSlice::new(input, 1, frames_in).context("Eingabepuffer ungueltig")?;
    let mut adapter_out =
        InterleavedSlice::new_mut(&mut output, 1, capacity).context("Ausgabepuffer ungueltig")?;

    let mut indexing = Indexing::new();
    let mut frames_left = frames_in;
    let mut next = resampler.input_frames_next();
    while frames_left >= next {
        let (used, produced) = resampler
            .process_into_buffer(&adapter_in, &mut adapter_out, Some(&indexing))
            .context("Resampling fehlgeschlagen")?;
        indexing.input_offset += used;
        indexing.output_offset += produced;
        frames_left -= used;
        next = resampler.input_frames_next();
    }
    indexing.partial_len = Some(frames_left);
    resampler
        .process_into_buffer(&adapter_in, &mut adapter_out, Some(&indexing))
        .context("Resampling des letzten Blocks fehlgeschlagen")?;

    let end = (delay + expected).min(output.len());
    Ok(output[delay.min(end)..end].to_vec())
}
