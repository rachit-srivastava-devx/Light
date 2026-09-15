use super::parsers;
use super::requirement::{ProbeTool, Requirement};
use super::spec::{GateCommand, GateSpec};

pub const GATES: &[GateSpec] = &[
    GateSpec {
        id: "unit tests",
        requirement: Requirement::Required,
        probe: ProbeTool::Cargo,
        command: GateCommand::OnPath(&["cargo", "test", "--workspace"]),
        parse_denominator: parsers::unit_tests,
    },
    GateSpec {
        id: "mutants",
        requirement: Requirement::Advisory,
        probe: ProbeTool::CargoMutants,
        command: GateCommand::OnPath(&["cargo", "mutants"]),
        parse_denominator: parsers::mutants,
    },
    GateSpec {
        id: "semgrep",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("semgrep"),
        command: GateCommand::Script {
            relative: "semgrep-gate.sh",
            args: &[],
        },
        parse_denominator: parsers::semgrep,
    },
    GateSpec {
        id: "trivy",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("trivy"),
        command: GateCommand::Script {
            relative: "trivy-gate.sh",
            args: &[],
        },
        parse_denominator: parsers::trivy,
    },
    GateSpec {
        id: "recur",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("bash"),
        command: GateCommand::Script {
            relative: "recur-gate.sh",
            args: &[],
        },
        parse_denominator: parsers::recur,
    },
    GateSpec {
        id: "detectors",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("bash"),
        command: GateCommand::Script {
            relative: "detector-integrity.sh",
            args: &[],
        },
        parse_denominator: parsers::detectors,
    },
    GateSpec {
        id: "policy",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("conftest"),
        command: GateCommand::Script {
            relative: "policy/run.sh",
            args: &[],
        },
        parse_denominator: parsers::policy,
    },
    GateSpec {
        id: "corpus",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("uv"),
        command: GateCommand::Script {
            relative: "corpus/run.sh",
            args: &[],
        },
        parse_denominator: parsers::corpus,
    },
];
