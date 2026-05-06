// Feature extraction for wake-word detection.
//
// Takes a slice of mono 16 kHz f32 PCM and returns a normalized
// log-magnitude spectrogram. We deliberately avoid a full mel filterbank
// to keep the dependency footprint small; cosine similarity over
// log-magnitude FFT bins is good enough to discriminate a handful of
// user-trained wake-word templates.

use rustfft::{num_complex::Complex32, FftPlanner};

pub const SAMPLE_RATE: u32 = 16_000;
const FRAME_SIZE: usize = 400; // 25 ms @ 16 kHz
const HOP_SIZE: usize = 160; //  10 ms @ 16 kHz
const N_BINS: usize = 32; // log-spaced bands kept per frame

#[derive(Debug, Clone)]
pub struct FeatureMatrix {
    pub frames: usize,
    pub bins: usize,
    /// Row-major: features[frame * bins + bin].
    pub data: Vec<f32>,
}

impl FeatureMatrix {
    pub fn frame(&self, i: usize) -> &[f32] {
        &self.data[i * self.bins..(i + 1) * self.bins]
    }

    pub fn is_empty(&self) -> bool {
        self.frames == 0
    }
}

/// Compute log-spectrogram features for `samples` (mono 16 kHz).
///
/// Returns an empty matrix if the input is too short for a single frame.
pub fn extract_features(samples: &[f32]) -> FeatureMatrix {
    if samples.len() < FRAME_SIZE {
        return FeatureMatrix { frames: 0, bins: N_BINS, data: Vec::new() };
    }

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FRAME_SIZE);
    let window = hann_window(FRAME_SIZE);

    let n_frames = (samples.len() - FRAME_SIZE) / HOP_SIZE + 1;
    let mut data = Vec::with_capacity(n_frames * N_BINS);

    let mut buf = vec![Complex32::new(0.0, 0.0); FRAME_SIZE];
    let bin_edges = log_bin_edges(FRAME_SIZE / 2, N_BINS);

    for f in 0..n_frames {
        let start = f * HOP_SIZE;
        let frame = &samples[start..start + FRAME_SIZE];
        for (i, x) in frame.iter().enumerate() {
            buf[i] = Complex32::new(*x * window[i], 0.0);
        }
        fft.process(&mut buf);

        // Magnitude spectrum (only the first half is unique for real input).
        let mut mag = vec![0.0f32; FRAME_SIZE / 2];
        for i in 0..FRAME_SIZE / 2 {
            mag[i] = (buf[i].norm_sqr()).sqrt();
        }

        // Sum into log-spaced bins.
        for b in 0..N_BINS {
            let lo = bin_edges[b];
            let hi = bin_edges[b + 1].max(lo + 1);
            let mut acc = 0.0f32;
            for v in &mag[lo..hi] {
                acc += *v;
            }
            data.push((acc + 1e-6).ln());
        }

        // Per-frame mean-normalize so loud vs. quiet doesn't dominate.
        let frame_slice = &mut data[f * N_BINS..(f + 1) * N_BINS];
        let mean = frame_slice.iter().sum::<f32>() / N_BINS as f32;
        let mut norm = 0.0f32;
        for v in frame_slice.iter_mut() {
            *v -= mean;
            norm += *v * *v;
        }
        let norm = norm.sqrt().max(1e-6);
        for v in frame_slice.iter_mut() {
            *v /= norm;
        }
    }

    FeatureMatrix { frames: n_frames, bins: N_BINS, data }
}

fn hann_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = (i as f32) / ((n - 1) as f32);
            0.5 - 0.5 * (2.0 * std::f32::consts::PI * t).cos()
        })
        .collect()
}

/// Logarithmically spaced bin edges over `[1, max_bin]`, length `n_bins+1`.
fn log_bin_edges(max_bin: usize, n_bins: usize) -> Vec<usize> {
    let lo = 1.0f32.ln();
    let hi = (max_bin as f32).ln();
    (0..=n_bins)
        .map(|i| {
            let t = i as f32 / n_bins as f32;
            (lo + (hi - lo) * t).exp().round() as usize
        })
        .map(|x| x.min(max_bin))
        .collect()
}

/// Resample a single-channel f32 buffer to `SAMPLE_RATE` using linear
/// interpolation. Good enough for narrowband wake-word features.
pub fn resample_to_target(input: &[f32], src_rate: u32) -> Vec<f32> {
    if src_rate == SAMPLE_RATE || input.is_empty() {
        return input.to_vec();
    }
    let ratio = SAMPLE_RATE as f64 / src_rate as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let i0 = src_pos.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let t = (src_pos - i0 as f64) as f32;
        let a = input[i0.min(input.len() - 1)];
        let b = input[i1];
        out.push(a + (b - a) * t);
    }
    out
}

/// Downmix interleaved multi-channel f32 audio to mono by averaging.
pub fn downmix_to_mono(input: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return input.to_vec();
    }
    let ch = channels as usize;
    let frames = input.len() / ch;
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut acc = 0.0f32;
        for c in 0..ch {
            acc += input[f * ch + c];
        }
        out.push(acc / ch as f32);
    }
    out
}
