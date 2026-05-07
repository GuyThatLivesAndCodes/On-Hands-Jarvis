// Wake-word detection: a lightweight DTW match between live audio
// features and the user-recorded templates.
//
// We compare the candidate window against:
//   * "positive" templates (recordings of the user saying the wake word),
//     and
//   * "negative" templates (background speech, noise, near-miss words).
//
// The wake fires when the best positive score exceeds the threshold AND
// beats the best negative score by a clear margin. With even a handful
// of negative samples this kills most everyday noise false-positives.

use crate::config::WakeTemplate;
use crate::voice::features::{extract_features, FeatureMatrix};

/// Margin the positive score has to beat the negative score by.
const MIN_NET_MARGIN: f32 = 0.05;

#[derive(Debug, Clone, Copy)]
pub struct WakeDecision {
    pub positive_score: f32,
    pub negative_score: f32,
    pub hit: bool,
}

pub struct WakeDetector {
    positives: Vec<FeatureMatrix>,
    negatives: Vec<FeatureMatrix>,
    pub threshold: f32,
}

impl WakeDetector {
    pub fn new(positives: &[WakeTemplate], negatives: &[WakeTemplate], threshold: f32) -> Self {
        let to_matrix = |t: &WakeTemplate| FeatureMatrix {
            frames: t.frames,
            bins: t.bins,
            data: t.features.clone(),
        };
        Self {
            positives: positives.iter().map(to_matrix).collect(),
            negatives: negatives.iter().map(to_matrix).collect(),
            threshold,
        }
    }

    pub fn score(&self, samples: &[f32]) -> WakeDecision {
        if self.positives.is_empty() {
            return WakeDecision { positive_score: 0.0, negative_score: 0.0, hit: false };
        }
        let candidate = extract_features(samples);
        if candidate.is_empty() {
            return WakeDecision { positive_score: 0.0, negative_score: 0.0, hit: false };
        }
        let positive_score = self
            .positives
            .iter()
            .map(|t| dtw_cosine_similarity(&candidate, t))
            .fold(0.0f32, f32::max);
        let negative_score = self
            .negatives
            .iter()
            .map(|t| dtw_cosine_similarity(&candidate, t))
            .fold(0.0f32, f32::max);
        let hit = positive_score >= self.threshold
            && positive_score > negative_score + MIN_NET_MARGIN;
        WakeDecision { positive_score, negative_score, hit }
    }
}

/// Constrained DTW alignment + cosine similarity per aligned pair.
/// Returns a similarity in `[0, 1]`.
fn dtw_cosine_similarity(a: &FeatureMatrix, b: &FeatureMatrix) -> f32 {
    if a.is_empty() || b.is_empty() || a.bins != b.bins {
        return 0.0;
    }
    let n = a.frames;
    let m = b.frames;
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
