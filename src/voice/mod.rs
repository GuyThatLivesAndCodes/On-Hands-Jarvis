// Voice subsystem: audio capture (cpal), feature extraction (FFT-based
// log-spectrogram), and wake-word matching against user-recorded templates.
//
// Public surface:
//   * `Recorder`     — start/stop a microphone stream, exposes a ring buffer
//                      of recent samples at a fixed sample rate.
//   * `extract_features` — turn raw mono 16 kHz audio into a feature matrix.
//   * `match_score`  — cosine-similarity match between a candidate feature
//                      matrix and a stored template.

pub mod features;
pub mod recorder;
pub mod wake;

pub use features::extract_features;
pub use recorder::{list_input_devices, list_output_devices, Recorder};
pub use wake::WakeDetector;
