// Microphone capture using cpal. Maintains a ring buffer of the last
// `buffer_seconds` of mono 16 kHz audio so callers can grab the most
// recent window for wake-word matching or one-shot recording.

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

use super::features::{downmix_to_mono, resample_to_target, SAMPLE_RATE};

pub struct Recorder {
    inner: Arc<Mutex<RingBuffer>>,
    _stream: Stream,
}

struct RingBuffer {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl RingBuffer {
    fn push(&mut self, slice: &[f32]) {
        for s in slice {
            if self.samples.len() == self.capacity {
                self.samples.pop_front();
            }
            self.samples.push_back(*s);
        }
    }

    fn snapshot(&self) -> Vec<f32> {
        self.samples.iter().copied().collect()
    }

    fn last_secs(&self, secs: f32) -> Vec<f32> {
        let n = ((secs * SAMPLE_RATE as f32) as usize).min(self.samples.len());
        self.samples
            .iter()
            .skip(self.samples.len() - n)
            .copied()
            .collect()
    }
}

impl Recorder {
    pub fn start(buffer_seconds: f32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device available"))?;

        let supported = device
            .default_input_config()
            .context("query default input config")?;
        let src_rate = supported.sample_rate().0;
        let channels = supported.channels();
        let sample_fmt = supported.sample_format();
        let config: StreamConfig = supported.into();

        let capacity = (buffer_seconds * SAMPLE_RATE as f32) as usize;
        let inner = Arc::new(Mutex::new(RingBuffer { samples: VecDeque::with_capacity(capacity), capacity }));
        let inner_cb = inner.clone();

        let err_cb = |e| log::warn!("audio stream error: {e}");

        let stream = match sample_fmt {
            SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    let mono = downmix_to_mono(data, channels);
                    let resampled = resample_to_target(&mono, src_rate);
                    inner_cb.lock().push(&resampled);
                },
                err_cb,
                None,
            )?,
            SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    let mut tmp = Vec::with_capacity(data.len());
                    for s in data {
                        tmp.push(*s as f32 / i16::MAX as f32);
                    }
                    let mono = downmix_to_mono(&tmp, channels);
                    let resampled = resample_to_target(&mono, src_rate);
                    inner_cb.lock().push(&resampled);
                },
                err_cb,
                None,
            )?,
            SampleFormat::U16 => device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    let mut tmp = Vec::with_capacity(data.len());
                    for s in data {
                        tmp.push((*s as f32 - 32768.0) / 32768.0);
                    }
                    let mono = downmix_to_mono(&tmp, channels);
                    let resampled = resample_to_target(&mono, src_rate);
                    inner_cb.lock().push(&resampled);
                },
                err_cb,
                None,
            )?,
            other => return Err(anyhow!("unsupported sample format: {other:?}")),
        };

        stream.play().context("start audio input stream")?;
        log::info!(
            "audio capture started: src_rate={} ch={} fmt={:?} -> {}Hz mono",
            src_rate,
            channels,
            sample_fmt,
            SAMPLE_RATE
        );

        Ok(Self { inner, _stream: stream })
    }

    /// Snapshot of the entire ring buffer (mono 16 kHz f32).
    pub fn snapshot(&self) -> Vec<f32> {
        self.inner.lock().snapshot()
    }

    /// Last `secs` seconds of audio (mono 16 kHz f32).
    pub fn last_seconds(&self, secs: f32) -> Vec<f32> {
        self.inner.lock().last_secs(secs)
    }

    /// Record a fixed-length sample, blocking on the calling thread until
    /// enough audio has arrived in the ring buffer.
    pub fn record_for(&self, secs: f32) -> Vec<f32> {
        let target = (secs * SAMPLE_RATE as f32) as usize;
        let start = std::time::Instant::now();
        // Wait for the buffer to fill at least `target` fresh samples.
        let baseline = self.inner.lock().samples.len();
        loop {
            let now = self.inner.lock().samples.len();
            if now.saturating_sub(baseline) >= target || now >= self.inner.lock().capacity {
                break;
            }
            if start.elapsed().as_secs_f32() > secs + 1.0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        self.last_seconds(secs)
    }
}
