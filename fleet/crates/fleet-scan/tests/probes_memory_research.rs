//! Behavior-spec cases for `MemoryProbe` and `ResearchProbe`, against mock ports.

mod common;
use common::input;

use fleet_scan::{EnvFault, MemoryHit, MemoryPort, MemoryProbe, Probe, ProbeKind, ProbeOutcome, ResearchPort, ResearchProbe};

struct MockMemory(Vec<MemoryHit>);
impl MemoryPort for MockMemory {
    fn recall_similar(&self, _query: &str, _limit: usize) -> Result<Vec<MemoryHit>, EnvFault> {
        Ok(self.0.clone())
    }
}

#[test]
fn memory_probe_flags_high_similarity_hits() {
    let memory = MockMemory(vec![
        MemoryHit { text: "prior decision".into(), score: 0.9 },
        MemoryHit { text: "unrelated".into(), score: 0.1 },
    ]);
    let probe = MemoryProbe { memory: &memory };
    let out = probe.probe(&input("new decision"));
    match out {
        ProbeOutcome::Questions(qs) => assert_eq!(qs.len(), 1),
        ProbeOutcome::Fault(_) => panic!("expected questions"),
    }
    assert_eq!(probe.kind(), ProbeKind::Memory);
}

struct MockResearch(Vec<String>);
impl ResearchPort for MockResearch {
    fn search(&self, _query: &str) -> Result<Vec<String>, EnvFault> {
        Ok(self.0.clone())
    }
}

#[test]
fn research_probe_flags_empty_results() {
    let research = MockResearch(vec![]);
    let probe = ResearchProbe { research: &research };
    let out = probe.probe(&input("some obscure claim"));
    match out {
        ProbeOutcome::Questions(qs) => assert_eq!(qs.len(), 1),
        ProbeOutcome::Fault(_) => panic!("expected questions"),
    }
    assert_eq!(probe.kind(), ProbeKind::Research);
}

#[test]
fn research_probe_clears_when_results_found() {
    let research = MockResearch(vec!["ref".into()]);
    let probe = ResearchProbe { research: &research };
    let out = probe.probe(&input("well documented thing"));
    assert_eq!(out, ProbeOutcome::Questions(vec![]));
}
