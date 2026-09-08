//! Behavior-spec cases for `BusinessProbe` and `TechnicalProbe`, against mock ports.

mod common;
use common::input;

use fleet_scan::{BusinessProbe, CodebasePort, EnvFault, Probe, ProbeKind, ProbeOutcome, TechnicalProbe};

#[test]
fn business_probe_flags_missing_metric_and_audience() {
    let out = BusinessProbe.probe(&input("build a thing"));
    match out {
        ProbeOutcome::Questions(qs) => assert_eq!(qs.len(), 2),
        ProbeOutcome::Fault(_) => panic!("expected questions"),
    }
    assert_eq!(BusinessProbe.kind(), ProbeKind::Business);
}

#[test]
fn business_probe_clears_when_metric_and_audience_stated() {
    let out = BusinessProbe.probe(&input("improve success metric for our customer"));
    assert_eq!(out, ProbeOutcome::Questions(vec![]));
}

struct MockCodebase(bool);
impl CodebasePort for MockCodebase {
    fn symbol_exists(&self, _name: &str) -> Result<bool, EnvFault> {
        Ok(self.0)
    }
}

#[test]
fn technical_probe_flags_existing_symbol() {
    let codebase = MockCodebase(true);
    let probe = TechnicalProbe { codebase: &codebase };
    let out = probe.probe(&input("add a new FooBar_Handler"));
    match out {
        ProbeOutcome::Questions(qs) => assert_eq!(qs.len(), 1),
        ProbeOutcome::Fault(_) => panic!("expected questions"),
    }
    assert_eq!(probe.kind(), ProbeKind::Technical);
}

struct FaultingCodebase;
impl CodebasePort for FaultingCodebase {
    fn symbol_exists(&self, _name: &str) -> Result<bool, EnvFault> {
        Err(EnvFault::CredentialMissing("token".into()))
    }
}

#[test]
fn technical_probe_reports_fault() {
    let codebase = FaultingCodebase;
    let probe = TechnicalProbe { codebase: &codebase };
    let out = probe.probe(&input("add a new FooBar_Handler"));
    assert!(matches!(out, ProbeOutcome::Fault(EnvFault::CredentialMissing(_))));
}
