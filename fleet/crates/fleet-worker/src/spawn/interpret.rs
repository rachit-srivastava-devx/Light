//! Interprets a received fd-3 packet into a typed `LaneOutcome`. Ported from
//! `main.rs::validate_submission` plus the `done`/`refuse` body extraction `agent_command`'s
//! call sites do today.

use crate::outcome::LaneOutcome;
use crate::spawn::fd3;

pub fn interpret_fd3(parent_fd: std::os::raw::c_int) -> LaneOutcome {
    let Some((packet, _at_ceiling)) = fd3::recv(parent_fd) else {
        return LaneOutcome::EnvironmentFault { detail: "recv() on fd-3 failed".into() };
    };
    if packet.is_empty() {
        return LaneOutcome::EnvironmentFault {
            detail: "agent exited without an fd-3 result".into(),
        };
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&packet) else {
        return LaneOutcome::EnvironmentFault { detail: "invalid fd-3 result".into() };
    };
    if !fd3::validate_submission(&value) {
        return LaneOutcome::EnvironmentFault {
            detail: "fd-3 packet failed validate_submission".into(),
        };
    }
    let body = value.get("body").cloned().unwrap_or(serde_json::Value::Null);
    match value.get("kind").and_then(serde_json::Value::as_str) {
        Some("done") => LaneOutcome::Done {
            resolved_model: value.get("resolved_model").and_then(|v| v.as_str()).map(str::to_string),
            tokens: parse_tokens(&value),
            body,
        },
        Some("refuse") => LaneOutcome::Refused {
            reason: value.get("reason").and_then(|v| v.as_str()).unwrap_or("refused").to_string(),
        },
        _ => LaneOutcome::EnvironmentFault {
            detail: "fd-3 packet was a `note`, not a final result".into(),
        },
    }
}

fn parse_tokens(value: &serde_json::Value) -> Option<fleet_types::Tokens> {
    value.get("tokens").and_then(serde_json::Value::as_u64).map(fleet_types::Tokens::new)
}
