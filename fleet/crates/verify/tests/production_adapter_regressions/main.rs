use std::sync::{Arc, Mutex};

use verify::{
    FindingsProvider, GateCommand, GatesRoot, ProbeTool, ProcessOutput, ProcessRunner,
    ReviewedCandidate, SecretFinding, ToolProbe, assemble_gate_evidence,
    secret_scan_integrity_digest, verify_production,
};

const OVERRIDE_ARGV: &[&str] = &["override-gate", "--preserve-me"];

fn candidate(specs: &[verify::GateSpec]) -> ReviewedCandidate {
    verify::candidate_from_specs(
        "tree-under-test".into(),
        "acceptance-under-test".into(),
        specs,
    )
}

fn override_spec() -> verify::GateSpec {
    let mut spec = verify::GATES[0];
    spec.command = GateCommand::OnPath(OVERRIDE_ARGV);
    spec
}

struct EverythingAvailable;

impl ToolProbe for EverythingAvailable {
    fn available(&self, _tool: ProbeTool) -> bool {
        true
    }
}

struct RecordingRunner {
    stdout: String,
    commands: Arc<Mutex<Vec<Vec<String>>>>,
}

impl ProcessRunner for RecordingRunner {
    fn run(&self, command: &[&str]) -> ProcessOutput {
        self.commands
            .lock()
            .expect("command log lock")
            .push(command.iter().map(|part| (*part).to_string()).collect());
        ProcessOutput {
            exit_code: 0,
            stdout: self.stdout.clone(),
            stderr: String::new(),
        }
    }
}

struct RecordingFindings {
    scans: Arc<Mutex<u32>>,
    findings: Vec<SecretFinding>,
}

impl FindingsProvider for RecordingFindings {
    fn scan(&self, _tree_digest: &str) -> Result<Vec<SecretFinding>, verify::VerifyError> {
        *self.scans.lock().expect("scan count lock") += 1;
        Ok(self.findings.clone())
    }
}

#[path = "finding_tests.rs"]
mod finding_tests;
#[path = "output_tests.rs"]
mod output_tests;
#[path = "override_tests.rs"]
mod override_tests;
#[path = "receipt_tests.rs"]
mod receipt_tests;
