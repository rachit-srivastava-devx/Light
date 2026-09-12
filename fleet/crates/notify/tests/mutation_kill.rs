use notify::outbox::{enqueue, OutboxRecord, OutboxStore};
use notify::redact::{derive_idempotency_key, redact_pii};
use notify::{Grant, Notification, NotifyError};
use std::collections::HashMap;

struct Ms(HashMap<String, OutboxRecord>);
impl Ms { fn new() -> Self { Ms(HashMap::new()) } }
impl OutboxStore for Ms {
    fn insert(&mut self, r: OutboxRecord) -> Result<(), NotifyError> {
        self.0.insert(r.idempotency_key.clone(), r); Ok(())
    }
    fn contains_key(&self, k: &str) -> bool { self.0.contains_key(k) }
    fn len(&self) -> usize { self.0.len() }
}
fn notif(k: &str) -> Notification {
    Notification { task_id: "t".into(), transport: "x".into(), recipient: "r".into(),
        payload: "p".into(), idempotency_key: k.into(),
        grant: Grant { transport: "x".into(), recipient: "r".into(),
            scope: "s".into(), expiry: 0 } }
}

// OutboxStore::is_empty: pins == 0 boundary and both constant-replacement mutations
#[test]
fn is_empty_true_when_empty() { assert!(Ms::new().is_empty()); }
#[test]
fn is_empty_false_after_enqueue() {
    let mut s = Ms::new(); enqueue(&mut s, notif("k")).unwrap(); assert!(!s.is_empty());
}

// redact_pii: kills whole-function replacement ("xyzzy" / String::new())
#[test]
fn plain_text_unchanged() { assert_eq!(redact_pii("hello world"), "hello world"); }

// redact_pii: exact output pins every operator on the hot path
#[test]
fn simple_email_marker() { assert_eq!(redact_pii("alice@example.com"), "[REDACTED]"); }
#[test]
fn email_in_sentence_exact() {
    assert_eq!(redact_pii("hi alice@example.com done"), "hi [REDACTED] done");
}

// redact_pii: @ with empty local part must NOT redact (kills && vs || on line 14)
#[test]
fn bare_at_not_redacted() { assert_eq!(redact_pii("@example.com"), "@example.com"); }

// redact_pii: special chars in local part must stay in word (kills line 11 == vs != / || vs &&)
#[test]
fn underscore_local_redacted() { assert_eq!(redact_pii("a_b@x.com"), "[REDACTED]"); }
#[test]
fn dot_local_redacted() { assert_eq!(redact_pii("a.b@x.com"), "[REDACTED]"); }
#[test]
fn hyphen_local_redacted() { assert_eq!(redact_pii("a-b@x.com"), "[REDACTED]"); }

// redact_pii: underscore domain start (kills line 13 == vs != on next_is_word check)
#[test]
fn underscore_domain_start_redacted() { assert_eq!(redact_pii("u@_d.com"), "[REDACTED]"); }

// redact_pii: special chars inside domain must be consumed (kills line 16 == vs != / || vs &&)
#[test]
fn dash_domain_consumed() { assert_eq!(redact_pii("u@a-b.com"), "[REDACTED]"); }
#[test]
fn underscore_domain_consumed() { assert_eq!(redact_pii("u@a_b.com"), "[REDACTED]"); }

// redact_pii: kills is_alphanumeric→is_alphabetic on all 3 sites (digit in local/domain leaks PII)
#[test]
fn digit_local_part_redacted() { assert_eq!(redact_pii("user1@host.com"), "[REDACTED]"); }
#[test]
fn digit_domain_start_redacted() { assert_eq!(redact_pii("user@1host.com"), "[REDACTED]"); }
#[test]
fn digit_domain_mid_redacted() { assert_eq!(redact_pii("user@host1.com"), "[REDACTED]"); }
// derive_idempotency_key: kills String::new() and "xyzzy" return mutations
#[test]
fn idem_key_has_prefix() {
    assert!(derive_idempotency_key("t", "s", "r", "p").starts_with("idem-"));
}
#[test]
fn idem_key_differs_by_task() {
    assert_ne!(derive_idempotency_key("t1", "s", "r", "p"),
               derive_idempotency_key("t2", "s", "r", "p"));
}
