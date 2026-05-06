// AI integration. Today we wire up the xAI Grok chat completions API; the
// `Client` type is generic enough to swap in a different OpenAI-compatible
// provider by changing the base URL.

pub mod grok;

pub use grok::{ChatMessage, ChatRequest, Client, Role};
