//! Payload redaction and deterministic idempotency-key derivation.
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Replace email-like patterns (local@domain.tld) with [REDACTED].
pub fn redact_pii(payload: &str) -> String {
    let mut result = String::with_capacity(payload.len());
    let mut word = String::new();
    let mut chars = payload.chars().peekable();
    while let Some(ch) = chars.next() {
        let in_word = ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.';
        if ch == '@' {
            let next_is_word = chars.peek().map(|c| c.is_alphanumeric() || *c == '_').unwrap_or(false);
            if next_is_word && !word.is_empty() {
                while let Some(&dc) = chars.peek() {
                    if dc.is_alphanumeric() || dc == '.' || dc == '-' || dc == '_' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                result.push_str("[REDACTED]");
                word.clear();
            } else {
                result.push_str(&word);
                result.push('@');
                word.clear();
            }
        } else if in_word {
            word.push(ch);
        } else {
            result.push_str(&word);
            word.clear();
            result.push(ch);
        }
    }
    result.push_str(&word);
    result
}

/// Deterministic idempotency key from (task_id, transport, recipient, payload).
pub fn derive_idempotency_key(
    task_id: &str,
    transport: &str,
    recipient: &str,
    payload: &str,
) -> String {
    let mut h = DefaultHasher::new();
    task_id.hash(&mut h);
    transport.hash(&mut h);
    recipient.hash(&mut h);
    payload.hash(&mut h);
    format!("idem-{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn email_is_redacted() {
        let s = redact_pii("hello alice@example.com done");
        assert!(!s.contains("alice@example.com"));
    }
    #[test]
    fn key_is_deterministic() {
        let k1 = derive_idempotency_key("t1", "slack", "bob", "hi");
        let k2 = derive_idempotency_key("t1", "slack", "bob", "hi");
        assert_eq!(k1, k2);
    }
}
