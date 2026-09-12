use serde::Serialize;
use crate::CliError;

pub fn render<T: Serialize>(value: &T, json: bool) -> Result<String, CliError> {
    if json {
        serde_json::to_string(value).map_err(|e| CliError::Render(e.to_string()))
    } else {
        serde_json::to_string_pretty(value).map_err(|e| CliError::Render(e.to_string()))
    }
}
