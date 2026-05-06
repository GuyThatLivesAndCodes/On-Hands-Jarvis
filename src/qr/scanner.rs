// Capture each connected monitor and decode any QR codes visible on
// screen. The detected codes (content + screen-space quad) are returned
// to the UI which renders them as clickable outlines.

use anyhow::{Context, Result};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ScannedCode {
    pub content: String,
    /// Screen-space corners of the QR symbol, in pixels of the monitor it
    /// was found on. Order is top-left, top-right, bottom-right, bottom-left.
    pub corners: [(i32, i32); 4],
    pub monitor_index: usize,
    pub captured_at: Instant,
}

pub struct Scanner {
    pub last_scan: Option<Instant>,
    pub interval: Duration,
    pub codes: Vec<ScannedCode>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            last_scan: None,
            interval: Duration::from_millis(800),
            codes: Vec::new(),
        }
    }
}

impl Scanner {
    pub fn tick(&mut self) -> Result<bool> {
        if let Some(t) = self.last_scan {
            if t.elapsed() < self.interval {
                return Ok(false);
            }
        }
        self.last_scan = Some(Instant::now());
        self.codes = scan_screen()?;
        Ok(true)
    }
}

/// Scan all monitors once and return any QR codes found.
pub fn scan_screen() -> Result<Vec<ScannedCode>> {
    let monitors = xcap::Monitor::all().context("enumerate monitors")?;
    let mut found = Vec::new();
    for (mi, monitor) in monitors.iter().enumerate() {
        let img = match monitor.capture_image() {
            Ok(i) => i,
            Err(e) => {
                log::warn!("capture monitor {mi} failed: {e}");
                continue;
            }
        };
        let w = img.width();
        let h = img.height();
        let raw = img.as_raw(); // RGBA bytes
        if raw.len() < (w * h * 4) as usize {
            continue;
        }

        // Convert to greyscale lazily inside rqrr's preparer.
        let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(
            w as usize,
            h as usize,
            |x, y| {
                let i = (y * w as usize + x) * 4;
                let r = raw[i] as u32;
                let g = raw[i + 1] as u32;
                let b = raw[i + 2] as u32;
                ((r * 299 + g * 587 + b * 114) / 1000) as u8
            },
        );

        for grid in prepared.detect_grids() {
            let bounds = grid.bounds;
            let corners = [
                (bounds[0].x, bounds[0].y),
                (bounds[1].x, bounds[1].y),
                (bounds[2].x, bounds[2].y),
                (bounds[3].x, bounds[3].y),
            ];
            match grid.decode() {
                Ok((_meta, content)) => {
                    found.push(ScannedCode {
                        content,
                        corners,
                        monitor_index: mi,
                        captured_at: Instant::now(),
                    });
                }
                Err(e) => log::debug!("qr decode skipped: {e}"),
            }
        }
    }
    Ok(found)
}
