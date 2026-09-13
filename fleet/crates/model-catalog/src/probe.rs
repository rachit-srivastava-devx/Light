use std::collections::HashMap;
use std::time::Duration;

use crate::{Candidate, CapabilityReport, CapabilityState};

fn unknown_report(candidate: &Candidate, reason: &str) -> CapabilityReport {
    let mut caps = HashMap::new();
    caps.insert(
        "probe_result".into(),
        CapabilityState::Unknown {
            reason: reason.into(),
        },
    );
    CapabilityReport {
        candidate: candidate.clone(),
        capabilities: caps,
        timestamp: std::time::SystemTime::now(),
    }
}

pub fn probe(candidate: &Candidate, timeout: Duration) -> CapabilityReport {
    let mut child = match std::process::Command::new(&candidate.path).spawn() {
        Err(_) => return unknown_report(candidate, "spawn failed"),
        Ok(c) => c,
    };
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let _ = child.wait();
        let _ = tx.send(());
    });
    match rx.recv_timeout(timeout) {
        Ok(_) => unknown_report(candidate, "probe completed without output"),
        Err(_) => unknown_report(candidate, "timeout"),
    }
}
