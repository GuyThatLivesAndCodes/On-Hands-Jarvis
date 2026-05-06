// QR code scanning subsystem: grab the screen, decode any QR symbols, and
// expose them as clickable annotations to the rest of the app.

pub mod scanner;

pub use scanner::{ScannedCode, Scanner};
