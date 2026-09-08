//! Parses an OpenAI-dialect chat-completion response into `RawVerdict`. Any shape mismatch
//! becomes a `ModelError` -- this module never invents a default field.

use crate::errors::ModelError;
use crate::types::RawVerdict;
use serde_json::Value;

pub fn parse_response(body: &Value) -> Result<RawVerdict, ModelError> {
    let args_str = body
        .pointer("/choices/0/message/tool_calls/0/function/arguments")
        .and_then(Value::as_str)
        .ok_or_else(|| ModelError::new(format!("no tool_call arguments in response: {body}")))?;
    let args: Value = serde_json::from_str(args_str)
        .map_err(|e| ModelError::new(format!("tool_call arguments not valid JSON: {e}")))?;
    Ok(RawVerdict {
        label: str_field(&args, "label"),
        confidence_pct: args.get("confidence_pct").and_then(Value::as_u64).map(|v| v as u8),
        because: str_field(&args, "because"),
        abstain_why: str_field(&args, "abstain_why"),
    })
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}
