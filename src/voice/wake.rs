// Wake-word detection: a lightweight DTW match between live audio
// features and the user-recorded templates.
//
// We don't aim for state-of-the-art accuracy — the template approach is
// designed so that 10 user samples are enough to reliably gate a small
// number of false positives in a single user's voice on a single
// microphone.

use crate::config::WakeTemplate;
use crate::voice::features::{extract_features, FeatureMatrix};

pub struct WakeDetector {
    templates: Vec<FeatureMatrix>,
    pub threshold: f32,
}

impl WakeDetector {
    pub fn new(templates: &[WakeTemplate], threshold: f32) -> Self {
        let templates = templates
            .iter()
            .map(|t| FeatureMatrix {
                frames: t.frames,
                bins: t.bins,
                data: t.features.clone(),
            })
            .collect();
        Self { templates, threshold }
    }

    /// Score the given audio window against the stored templates.
    /// Returns the best score in `[0, 1]` and the matching template index
    /// if any score exceeded `threshold`.
    pub fn score(&self, samples: &[f32]) -> (f32, Option<usize>) {
        if self.templates.is_empty() {
            return (0.0, None);
        }
        let candidate = extract_features(samples);
        if candidate.is_empty() {
            return (0.0, None);
        }
        let mut best = 0.0f32;
        let mut best_idx = None;
        for (i, t) in self.templates.iter().enumerate() {
            let s = dtw_cosine_similarity(&candidate, t);
            if s > best {
                best = s;
                if s >= self.threshold {
                    best_idx = Some(i);
                }
            }
        }
        (best, best_idx)
    }
}

/// Convenience: identical to `WakeDetector::score` but built ad hoc.
pub fn best_match(samples: &[f32], templates: &[WakeTemplate], threshold: f32) -> (f32, Option<usize>) {
    WakeDetector::new(templates, threshold).score(samples)
}

/// Constrained DTW alignment + cosine similarity per aligned pair.
/// Returns a similarity in `[0, 1]`.
fn dtw_cosine_similarity(a: &FeatureMatrix, b: &FeatureMatrix) -> f32 {
    if a.is_empty() || b.is_empty() || a.bins != b.bins {
        return 0.0;
    }
    let n = a.frames;
    let m = b.frames;

    // Sakoe-Chiba band: width = max(|n-m|, 10) frames around the diagonal.
    let band = ((n as isize - m as isize).unsigned_abs() as usize).max(10);

    let inf = f32::INFINITY;
    let mut prev = vec![inf; m + 1];
    let mut curr = vec![inf; m + 1];
    prev[0] = 0.0;

    for i in 1..=n {
        for v in curr.iter_mut() {
            *v = inf;
        }
        let j_lo = i.saturating_sub(band).max(1);
        let j_hi = (i + band).min(m);
        for j in j_lo..=j_hi {
            // Cost = 1 - cosine_similarity (frames already L2-normalized).
            let dot = dot_product(a.frame(i - 1), b.frame(j - 1));
            let cost = 1.0 - dot;
            let best_prev = prev[j].min(prev[j - 1]).min(curr[j - 1]);
            curr[j] = cost + best_prev;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    let total = prev[m];
    if !total.is_finite() {
        return 0.0;
    }
    let path_len = (n + m) as f32;
    let avg_cost = total / path_len;
    (1.0 - avg_cost).clamp(0.0, 1.0)
}

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
