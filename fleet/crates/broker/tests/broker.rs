use broker::{
    BrokerError, EffectState, EffectStore, ExternalEffect, Grant, Provider, ProviderAck, Readback,
};
use std::{cell::Cell, collections::HashMap};

struct Mem(HashMap<String, EffectState>);
impl EffectStore for Mem {
    fn load(&self, k: &str) -> Option<EffectState> {
        self.0.get(k).cloned()
    }
    fn save(&mut self, k: &str, s: EffectState) -> Result<(), BrokerError> {
        self.0.insert(k.to_owned(), s);
        Ok(())
    }
}

struct Mock {
    ok: bool,
    calls: Cell<u32>,
}
impl Provider for Mock {
    fn dispatch(&self, e: &ExternalEffect) -> Result<ProviderAck, BrokerError> {
        self.calls.set(self.calls.get() + 1);
        if self.ok {
            Ok(ProviderAck { key: e.idempotency_key.clone(), provider_ref: "scripted".into() })
        } else {
            Err(BrokerError::Provider("scripted failure".into()))
        }
    }
    fn readback(&self, k: &str) -> Result<Readback, BrokerError> {
        Ok(Readback { key: k.into(), state: EffectState::Acked })
    }
}

fn base_effect() -> ExternalEffect {
    ExternalEffect {
        effect_id: "e1".into(),
        action: "push".into(),
        resource: "github:repo/branch".into(),
        payload_hash: "abc123".into(),
        idempotency_key: "key-1".into(),
        grant: Grant {
            action: "push".into(),
            resource: "github:repo/branch".into(),
            payload_hash: "abc123".into(),
            consumed: true,
            expires_at: i64::MAX,
        },
    }
}

mod tests {
    use super::*;
    use broker::execute;

    #[test]
    fn approval_consumption_produces_effect() {
        let p = Mock { ok: true, calls: Cell::new(0) };
        let mut s = Mem(HashMap::new());
        let r = execute(&p, &mut s, base_effect()).unwrap();
        assert_eq!(r.state, EffectState::Acked);
        assert_eq!(r.checked, 1);
        assert_eq!(r.total, 1);
    }

    #[test]
    fn provider_ack_advances_receipt() {
        let p = Mock { ok: true, calls: Cell::new(0) };
        let mut s = Mem(HashMap::new());
        let e = base_effect();
        execute(&p, &mut s, e.clone()).unwrap();
        let r = execute(&p, &mut s, e).unwrap();
        assert_eq!(r.state, EffectState::Acked);
        assert_eq!(p.calls.get(), 1, "provider called more than once for same key");
    }

    #[test]
    fn missing_approval_blocks_dispatch() {
        struct Never;
        impl Provider for Never {
            fn dispatch(&self, _: &ExternalEffect) -> Result<ProviderAck, BrokerError> {
                panic!("provider must not be called without a valid grant")
            }
            fn readback(&self, _: &str) -> Result<Readback, BrokerError> {
                panic!("provider must not be called without a valid grant")
            }
        }
        let mut s = Mem(HashMap::new());
        let mut e = base_effect();
        e.grant.consumed = false;
        assert!(matches!(execute(&Never, &mut s, e), Err(BrokerError::Grant(_))));
    }
}
