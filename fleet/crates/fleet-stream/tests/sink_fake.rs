//! `FakeSink`: a programmable `Sink` test double, shared by `fleet-stream`'s own suite.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use fleet_stream::{Sink, SinkError, StreamEvent};

/// A `Sink` whose `deliver` outcome per `seq` is programmable: fail `Transient` a configured
/// number of times, always fail `Permanent`, or succeed -- and records every successful/skipped
/// seq for assertion.
pub struct FakeSink {
    id: &'static str,
    delivered: Arc<Mutex<Vec<u64>>>,
    attempts: Mutex<HashMap<u64, u32>>,
    transient_until: HashMap<u64, u32>,
    permanent: HashSet<u64>,
}

impl FakeSink {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            delivered: Arc::new(Mutex::new(Vec::new())),
            attempts: Mutex::new(HashMap::new()),
            transient_until: HashMap::new(),
            permanent: HashSet::new(),
        }
    }

    pub fn with_transient(mut self, seq: u64, times: u32) -> Self {
        self.transient_until.insert(seq, times);
        self
    }

    pub fn with_permanent(mut self, seq: u64) -> Self {
        self.permanent.insert(seq);
        self
    }

    pub fn delivered_handle(&self) -> Arc<Mutex<Vec<u64>>> {
        Arc::clone(&self.delivered)
    }
}

impl Sink for FakeSink {
    fn id(&self) -> &'static str {
        self.id
    }

    fn accepts(&self, _event: &StreamEvent) -> bool {
        true
    }

    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError> {
        let seq = event.seq();
        if self.permanent.contains(&seq) {
            return Err(SinkError::Permanent { sink: self.id, seq, reason: "fake permanent".into() });
        }
        let mut attempts = self.attempts.lock().unwrap();
        let count = attempts.entry(seq).or_insert(0);
        if let Some(&limit) = self.transient_until.get(&seq) {
            if *count < limit {
                *count += 1;
                return Err(SinkError::Transient { sink: self.id, reason: "fake transient".into() });
            }
        }
        drop(attempts);
        self.delivered.lock().unwrap().push(seq);
        Ok(())
    }
}
