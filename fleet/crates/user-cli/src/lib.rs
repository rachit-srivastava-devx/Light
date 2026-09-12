use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod parse;
pub mod render;

pub use parse::parse;
pub use render::render;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CliCommand {
    Submit { request: String },
    Pause,
    Resume,
    Cancel,
    Approve { grant: String },
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliEvent {
    pub request_id: String,
    pub actor: String,
    pub command: CliCommand,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("empty request")]
    EmptyRequest,
    #[error("request too large: {bytes} bytes")]
    TooLarge { bytes: usize },
    #[error("invalid grant")]
    InvalidGrant,
    #[error("unknown command")]
    UnknownCommand,
    #[error("render error: {0}")]
    Render(String),
}

pub fn to_event(
    command: CliCommand,
    request_id: String,
    actor: String,
) -> Result<CliEvent, CliError> {
    Ok(CliEvent { request_id, actor, command })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_empty_submit() {
        assert!(matches!(parse(["submit", ""]), Err(CliError::EmptyRequest)));
    }

    #[test]
    fn parse_rejects_oversized_request() {
        let big = "x".repeat(65537);
        let result = parse(["submit", &big]);
        assert!(matches!(result, Err(CliError::TooLarge { bytes: 65537 })));
    }

    #[test]
    fn render_is_valid_json() {
        let event = CliEvent {
            request_id: "r1".to_string(),
            actor: "a1".to_string(),
            command: CliCommand::Status,
        };
        let s = render(&event, true).expect("render ok");
        let v: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
        assert!(v.get("request_id").is_some() && v.get("actor").is_some());
    }
}
