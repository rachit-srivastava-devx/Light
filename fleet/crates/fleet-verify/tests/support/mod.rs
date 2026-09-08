//! Shared fakes for `ToolProbe`/`ProcessRunner`, used by the orchestration/classify test files.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::BTreeSet;

use fleet_verify::{GatesRoot, ProbeTool, ProcessOutput, ProcessRunner, ToolProbe};

pub struct FakeProbe {
    pub available: BTreeSet<ProbeTool>,
}

impl ToolProbe for FakeProbe {
    fn available(&self, tool: ProbeTool) -> bool {
        self.available.contains(&tool)
    }
}

/// Returns one scripted `ProcessOutput` per call, in order; records every argv it was asked to
/// run so tests can assert whether a gate's command was actually invoked.
pub struct FakeRunner {
    pub scripted: RefCell<Vec<ProcessOutput>>,
    pub calls: RefCell<Vec<Vec<String>>>,
}

impl FakeRunner {
    pub fn new(scripted: Vec<ProcessOutput>) -> Self {
        Self {
            scripted: RefCell::new(scripted),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl ProcessRunner for FakeRunner {
    fn run(&self, command: &[&str]) -> ProcessOutput {
        self.calls
            .borrow_mut()
            .push(command.iter().map(|s| s.to_string()).collect());
        if self.scripted.borrow().is_empty() {
            return ProcessOutput {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
            };
        }
        self.scripted.borrow_mut().remove(0)
    }
}

/// A `GatesRoot` for tests that only exercise `GateCommand::OnPath` gates -- its content never
/// gets read (`GatesRoot::require` is never called), it just needs to exist.
pub fn dummy_gates() -> GatesRoot {
    GatesRoot::from_override(std::env::temp_dir()).expect("temp dir always exists")
}

pub fn out(exit_code: i32, stdout: &str) -> ProcessOutput {
    ProcessOutput {
        exit_code,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}
