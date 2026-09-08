//! `OtelSink`'s stdin payload shape, split out to keep `otel.rs` under the line cap.

use serde_json::{json, Value};

use crate::event::StreamEvent;

/// Per §5's field mapping: `event`, `component`, `task_id`, `attempt`, `exit_code`,
/// `token_source`, `model`, `provider`, `tokens_in`, `tokens_out`, `duration_ms`.
pub(super) fn payload(event: &StreamEvent) -> Value {
    let body = event.0.body.as_object();
    let get = |key: &str| body.and_then(|b| b.get(key)).cloned().unwrap_or(Value::Null);
    json!({
        "event": format!("{:?}", event.0.event),
        "component": get("component"),
        "task_id": get("task_id"),
        "attempt": get("attempt"),
        "exit_code": event.0.exit_code.map(|code| code.as_i32()).unwrap_or(0),
        "token_source": get("token_source"),
        "model": event.0.resolved_model.clone(),
        "provider": get("provider"),
        "tokens_in": get("tokens_in"),
        "tokens_out": get("tokens_out"),
        "duration_ms": get("duration_ms"),
    })
}
