use notify::outbox::{emit_notification, enqueue, OutboxRecord, OutboxStore};
use notify::reconcile::{build_delivery_receipt, handle_delivery_failure, NotifyInput};
use notify::transport::StateChangeEvent;
use notify::{DeliveryStatus, Grant, Notification, NotifyError};
use std::collections::HashMap;

struct MemStore(HashMap<String, OutboxRecord>);
impl MemStore {
    fn new() -> Self {
        MemStore(HashMap::new())
    }
}
impl OutboxStore for MemStore {
    fn insert(&mut self, r: OutboxRecord) -> Result<(), NotifyError> {
        self.0.insert(r.idempotency_key.clone(), r);
        Ok(())
    }
    fn contains_key(&self, k: &str) -> bool {
        self.0.contains_key(k)
    }
    fn len(&self) -> usize {
        self.0.len()
    }
}

fn make_grant() -> Grant {
    Grant {
        transport: "terminal".into(),
        recipient: "ops".into(),
        scope: "task".into(),
        expiry: 9999,
    }
}

mod tests {
    use super::*;

    #[test]
    fn state_change_emits_redacted_notification() {
        let event = StateChangeEvent {
            task_id: "t1".into(),
            state: "done".into(),
            payload: "task finished successfully".into(),
            user_email: String::new(),
        };
        let (n, receipt) = emit_notification(
            &mut MemStore::new(),
            &event,
            "terminal",
            "ops",
            make_grant(),
        )
        .unwrap();
        assert!(!n.payload.is_empty(), "payload must be present");
        assert!(receipt.checked > 0, "checked must be nonzero");
        assert!(receipt.total > 0, "total must be nonzero");
    }

    #[test]
    fn sensitive_fields_not_in_notification() {
        let event = StateChangeEvent {
            task_id: "t2".into(),
            state: "done".into(),
            payload: "user alice@example.com completed task".into(),
            user_email: "alice@example.com".into(),
        };
        let (n, _) = emit_notification(
            &mut MemStore::new(),
            &event,
            "terminal",
            "ops",
            make_grant(),
        )
        .unwrap();
        assert!(
            !n.payload.contains("alice@example.com"),
            "PII must not appear in notification payload"
        );
    }

    #[test]
    fn duplicate_delivery_refused() {
        let mut store = MemStore::new();
        let n = Notification {
            task_id: "t3".into(),
            transport: "terminal".into(),
            recipient: "ops".into(),
            payload: "hi".into(),
            idempotency_key: "key-xyz".into(),
            grant: make_grant(),
        };
        enqueue(&mut store, n.clone()).unwrap();
        let result = enqueue(&mut store, n);
        assert!(
            matches!(result, Err(NotifyError::Conflict)),
            "second enqueue must return Conflict"
        );
        assert_eq!(store.len(), 1, "outbox must have exactly one record");
    }

    #[test]
    fn zero_denominator_vacuous_proof_refused() {
        let input = NotifyInput {
            effect_id: "eff-1".into(),
            delivery_attempts: 0,
            total_recipients: 0,
        };
        let result = build_delivery_receipt(input);
        assert!(result.is_err(), "zero denominator must return Err");
        assert!(matches!(result, Err(NotifyError::ZeroDenominator)));
    }

    #[test]
    fn failed_delivery_does_not_panic() {
        let err = NotifyError::Transport("timeout".into());
        let receipt = handle_delivery_failure("eff-5", &err);
        assert_eq!(
            receipt.status,
            DeliveryStatus::Unknown,
            "failure must yield Unknown, not Delivered"
        );
    }
}
