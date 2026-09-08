//! Orchestration: run all 4 probes via the injected runner, then merge.

use crate::fault::EnvFault;
use crate::input::{ProbeKind, RequirementInput};
use crate::merge::merge_questions;
use crate::open_questions::Assessment;
use crate::ports::{ConcurrentRunner, ProbeRun};
use crate::probe::{Probe, ProbeOutcome};

/// The 4 probes bundled for one `assess` call. A struct (not 4 positional args) so the fixed
/// business/technical/memory/research order is named at every call site, not positional.
pub struct ProbeSet<'a> {
    pub business: &'a dyn Probe,
    pub technical: &'a dyn Probe,
    pub memory: &'a dyn Probe,
    pub research: &'a dyn Probe,
}

/// The full result of one `assess` call: the ADHD-safe verdict, plus every probe fault that
/// occurred along the way (for logs/observability -- never shown to the ADHD-safe surface, which
/// only ever sees `result`).
#[derive(Clone, Debug, PartialEq)]
pub struct AssessmentReport {
    pub result: Assessment,
    pub faults: Vec<(ProbeKind, EnvFault)>,
}

const KINDS: [ProbeKind; 4] = [
    ProbeKind::Business,
    ProbeKind::Technical,
    ProbeKind::Memory,
    ProbeKind::Research,
];

/// Run all 4 probes concurrently via `runner` (never spawning anything itself), collect whatever
/// questions/faults they produced, and merge per `merge_questions`. A fault or a caught panic in
/// any one probe contributes zero questions and one entry to `faults` -- it never prevents the
/// other 3 probes' questions from reaching the merge, and `assess` is infallible by design.
pub fn assess(
    input: &RequirementInput,
    probes: &ProbeSet<'_>,
    runner: &dyn ConcurrentRunner,
) -> AssessmentReport {
    let probe_refs: [&dyn Probe; 4] = [probes.business, probes.technical, probes.memory, probes.research];
    let jobs = probe_refs.map(|p| {
        let job: crate::ports::ProbeJob<'_> = Box::new(move || p.probe(input));
        job
    });
    let runs = runner.run4(jobs);

    let mut candidates = Vec::new();
    let mut faults = Vec::new();
    for (kind, run) in KINDS.into_iter().zip(runs) {
        match run {
            ProbeRun::Completed(ProbeOutcome::Questions(qs)) => candidates.extend(qs),
            ProbeRun::Completed(ProbeOutcome::Fault(f)) => faults.push((kind, f)),
            ProbeRun::Panicked(msg) => faults.push((kind, EnvFault::Internal(msg))),
        }
    }
    AssessmentReport { result: merge_questions(candidates), faults }
}
