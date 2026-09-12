//! Real keyless adapter: `POST https://api.llm7.io/v1/chat/completions`, model
//! `codestral-latest`, OpenAI dialect, forced tool call for a structured verdict instead of
//! parsing prose. Verified this session: no API key, no account, well-formed `tool_calls`.
//!
//! Kept behind the `llm7` feature so the pure core (`judge`, `Verdict`, `JudgeModel`) never
//! depends on an HTTP client.

mod attempt;
mod client;
mod parse;
mod schema;

pub use client::Llm7Judge;

pub const DEFAULT_ENDPOINT: &str = "https://api.llm7.io/v1/chat/completions";
pub const MODEL: &str = "codestral-latest";
