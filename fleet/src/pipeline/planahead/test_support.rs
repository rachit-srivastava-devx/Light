//! Shared test doubles for `orchestrator_tests.rs`, split out to keep each test file under the
//! 80-line gate. Every helper here is deterministic-by-synchronization: gates are real
//! `std::sync::mpsc` channels (blocking, used from inside `spawn_blocking`), never a sleep
//! guessed to be "long enough".

use crate::pipeline::planahead::UnitStep;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
pub struct EventLog(Arc<Mutex<Vec<String>>>);

impl EventLog {
    pub fn push(&self, event: impl Into<String>) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).push(event.into());
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn contains(&self, event: &str) -> bool {
        self.snapshot().iter().any(|e| e == event)
    }
}

pub fn plan_fn(log: EventLog) -> UnitStep {
    Arc::new(move |unit| {
        log.push(format!("plan:{unit}"));
        Ok(())
    })
}

/// A build step that blocks `gated_unit` on `release_rx` (signalling `started_tx` first) and
/// completes every other unit immediately. Standing in for "one slow build lane".
pub fn gated_build_fn(
    log: EventLog,
    gated_unit: &'static str,
    started_tx: std::sync::mpsc::Sender<()>,
    release_rx: std::sync::mpsc::Receiver<()>,
) -> UnitStep {
    let release_rx = Arc::new(Mutex::new(release_rx));
    Arc::new(move |unit| {
        log.push(format!("build_start:{unit}"));
        if unit == gated_unit {
            let _ = started_tx.send(());
            let _ = release_rx.lock().unwrap_or_else(|p| p.into_inner()).recv_timeout(Duration::from_secs(10));
        }
        log.push(format!("build_end:{unit}"));
        Ok(())
    })
}

/// Polls (bounded, not a guessed sleep) until `pred` is true or the deadline passes, returning
/// whether it converged -- used only to wait for an event to exist, never to assert timing.
pub async fn wait_until(mut pred: impl FnMut() -> bool) -> bool {
    for _ in 0..500 {
        if pred() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}
