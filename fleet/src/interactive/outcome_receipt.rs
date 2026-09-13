//! Durable, parent-owned accounting for one interactive adapter attempt.

use std::path::Path;

pub(super) fn record(
    state_dir: &Path,
    agent: &str,
    body: &serde_json::Value,
    resolved_model: Option<&str>,
    tokens: Option<u64>,
    ok: bool,
) {
    let receipt = serde_json::json!({
        "agent": agent,
        "ok": ok,
        "reason": body.get("reason"),
        "tokens": tokens,
        "token_usage": body.get("token_usage"),
    });
    if let Err(error) = crate::pipeline::ledger_events::append_as_with_model(
        state_dir,
        types::ReceiptEvent::LaneStatus,
        receipt,
        "fleet-cli-interactive",
        resolved_model,
    ) {
        eprintln!("fleet: could not record agent outcome: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::record;
    use store::ledger::LedgerPaths;
    use store::Ledger;

    #[test]
    fn records_provider_model_and_reported_tokens_as_a_parent_receipt() {
        let temp = tempfile::tempdir().unwrap();
        record(
            temp.path(),
            "claude",
            &serde_json::json!({"token_usage":{"input_tokens":3,"output_tokens":2}}),
            Some("claude-sonnet-4"),
            Some(5),
            true,
        );
        let ledger = Ledger::open(LedgerPaths {
            chain: temp.path().join("ledger.chain"),
            lock: temp.path().join("ledger.lock"),
        });
        let rows = ledger.rows(false).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].resolved_model.as_deref(), Some("claude-sonnet-4"));
        assert_eq!(rows[0].body["tokens"], 5);
    }
}
