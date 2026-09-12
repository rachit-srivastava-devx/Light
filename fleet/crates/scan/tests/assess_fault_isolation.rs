//! `assess()` fault- and panic-isolation: one bad probe never blocks the other 3.

mod common;
use common::{input, qw, FixedProbe, SequentialRunner};

use fleet_scan::{assess, Assessment, ConcurrentRunner, EnvFault, GapSeverity, ProbeJob, ProbeKind, ProbeOutcome, ProbeRun, ProbeSet};

#[test]
fn one_faulted_probe_never_blocks_the_others() {
    let business = FixedProbe {
        kind: ProbeKind::Business,
        outcome: ProbeOutcome::Questions(vec![qw("business q", GapSeverity::High, ProbeKind::Business)]),
    };
    let technical = FixedProbe {
        kind: ProbeKind::Technical,
        outcome: ProbeOutcome::Questions(vec![qw("technical q", GapSeverity::Medium, ProbeKind::Technical)]),
    };
    let memory = FixedProbe {
        kind: ProbeKind::Memory,
        outcome: ProbeOutcome::Questions(vec![qw("memory q", GapSeverity::Low, ProbeKind::Memory)]),
    };
    let research = FixedProbe {
        kind: ProbeKind::Research,
        outcome: ProbeOutcome::Fault(EnvFault::NetworkUnavailable("offline".into())),
    };
    let probes = ProbeSet { business: &business, technical: &technical, memory: &memory, research: &research };
    let report = assess(&input("requirement"), &probes, &SequentialRunner);

    assert_eq!(report.faults, vec![(ProbeKind::Research, EnvFault::NetworkUnavailable("offline".into()))]);
    match report.result {
        Assessment::Open(open) => assert_eq!(open.as_slice().len(), 3),
        Assessment::Clear => panic!("expected the 3 working probes' questions"),
    }
}

struct PanicOnResearchRunner;
impl ConcurrentRunner for PanicOnResearchRunner {
    fn run4<'a>(&self, jobs: [ProbeJob<'a>; 4]) -> [ProbeRun; 4] {
        let mut out = jobs.map(|job| ProbeRun::Completed(job()));
        out[3] = ProbeRun::Panicked("boom".into());
        out
    }
}

#[test]
fn panicking_probe_is_isolated_by_the_runner() {
    let business = FixedProbe {
        kind: ProbeKind::Business,
        outcome: ProbeOutcome::Questions(vec![qw("business q", GapSeverity::High, ProbeKind::Business)]),
    };
    let technical = FixedProbe { kind: ProbeKind::Technical, outcome: ProbeOutcome::Questions(vec![]) };
    let memory = FixedProbe { kind: ProbeKind::Memory, outcome: ProbeOutcome::Questions(vec![]) };
    let research = FixedProbe { kind: ProbeKind::Research, outcome: ProbeOutcome::Questions(vec![]) };
    let probes = ProbeSet { business: &business, technical: &technical, memory: &memory, research: &research };
    let report = assess(&input("requirement"), &probes, &PanicOnResearchRunner);

    assert_eq!(report.faults, vec![(ProbeKind::Research, EnvFault::Internal("boom".into()))]);
    match report.result {
        Assessment::Open(open) => assert_eq!(open.as_slice().len(), 1),
        Assessment::Clear => panic!("expected business probe's question to survive"),
    }
}
