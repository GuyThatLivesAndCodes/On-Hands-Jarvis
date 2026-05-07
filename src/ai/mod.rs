// AI integration. Today we wire up the xAI Grok chat completions API; the
// `Client` type is generic enough to swap in a different OpenAI-compatible
// provider by changing the base URL.

pub mod grok;
pub mod tools;

pub use grok::{ChatMessage, ChatRequest, Client, Role};
pub use tools::{available_tools, catalog_summary, execute as execute_tool, AgentContext};
