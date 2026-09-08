//! The committed gate table -- mirrors `verify.sh`'s `stage "name" req probe reason cmd...`
//! call list, retyped as data. Each entry maps one gate family from the reuse map (§5) to its
//! `ProbeTool`, requirement level, and stdout parser.

use crate::parsers;
use crate::requirement::{ProbeTool, Requirement};
use crate::spec::GateSpec;

pub const GATES: &[GateSpec] = &[
    GateSpec {
        id: "unit tests",
        requirement: Requirement::Required,
        probe: ProbeTool::Cargo,
        command: &["cargo", "test", "--workspace"],
        parse_denominator: parsers::unit_tests,
    },
    GateSpec {
        id: "mutants",
        requirement: Requirement::Advisory,
        probe: ProbeTool::CargoMutants,
        command: &["cargo", "mutants"],
        parse_denominator: parsers::mutants,
    },
    GateSpec {
        id: "semgrep",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("semgrep"),
        command: &["bin/semgrep-gate.sh"],
        parse_denominator: parsers::semgrep,
    },
    GateSpec {
        id: "trivy",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("trivy"),
        command: &["bin/trivy-gate.sh"],
        parse_denominator: parsers::trivy,
    },
    GateSpec {
        id: "recur",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("bash"),
        command: &["bin/recur-gate.sh"],
        parse_denominator: parsers::recur,
    },
    GateSpec {
        id: "detectors",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("bash"),
        command: &["bin/detector-integrity.sh"],
        parse_denominator: parsers::detectors,
    },
    GateSpec {
        id: "policy",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("conftest"),
        command: &["policy/run.sh"],
        parse_denominator: parsers::policy,
    },
    GateSpec {
        id: "corpus",
        requirement: Requirement::Required,
        probe: ProbeTool::Named("uv"),
        command: &["tests/corpus/run.sh"],
        parse_denominator: parsers::corpus,
    },
];
