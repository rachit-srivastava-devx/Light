//! Unit cover for `.fleet/gates.toml` resolution. The end-to-end property (a repo's own gate
//! commands really being spawned) is driven through the real binary in
//! `src/tests/run_json_reports_stages_and_gates.rs`: this proves the mapping, not the wiring.

use super::*;
use fleet_verify::Requirement;

fn write_config(repo: &Path, body: &str) {
    std::fs::create_dir_all(repo.join(".fleet")).unwrap();
    std::fs::write(file::path(repo), body).unwrap();
}

fn named<'a>(specs: &'a [GateSpec], id: &str) -> &'a GateSpec {
    specs.iter().find(|s| s.id == id).expect("gate present")
}
/// `GateSpec` is not `Debug`, so `expect_err` cannot be used on `resolve`'s `Result`.
fn err(repo: &Path, why: &str) -> DispatchError {
    resolve(repo).err().unwrap_or_else(|| panic!("{why}"))
}

fn argv(spec: &GateSpec) -> Vec<String> {
    match spec.command {
        GateCommand::OnPath(a) => a.iter().map(|s| s.to_string()).collect(),
        GateCommand::Script { relative, .. } => vec![format!("script:{relative}")],
    }
}


#[test]
fn no_config_file_returns_the_committed_table_unchanged() {
    let repo = tempfile::tempdir().unwrap();
    let specs = resolve(repo.path()).expect("an absent config is not an error");
    assert_eq!(specs.len(), fleet_verify::GATES.len());
    for (got, want) in specs.iter().zip(fleet_verify::GATES) {
        assert_eq!((got.id, argv(got), got.probe), (want.id, argv(want), want.probe));
    }
}

#[test]
fn a_node_repo_maps_the_unit_tests_gate_to_npm() {
    let repo = tempfile::tempdir().unwrap();
    let body = "[gates.\"unit tests\"]\ncommand = [\"npm\", \"run\", \"test:unit\"]\nprobe = \"npm\"\n";
    write_config(repo.path(), body);
    let specs = resolve(repo.path()).expect("a valid config resolves");
    let unit = named(&specs, "unit tests");
    assert_eq!(argv(unit), ["npm", "run", "test:unit"]);
    assert_eq!(unit.probe, ProbeTool::Named("npm"));
    // A config may not downgrade a gate: everything it did not name keeps its committed value.
    assert_eq!(unit.requirement, Requirement::Required);
    assert_eq!(argv(named(&specs, "semgrep")), ["script:semgrep-gate.sh"]);
}

#[test]
fn an_unknown_gate_id_is_an_environment_fault_not_a_no_op() {
    let repo = tempfile::tempdir().unwrap();
    write_config(repo.path(), "[gates.\"unit-tests\"]\ncommand = [\"npm\", \"test\"]\n");
    let fault = err(repo.path(), "a typo'd id must not be ignored");
    assert_eq!(fault.exit_code(), fleet_types::ExitCode::Env);
    let text = fault.to_string();
    assert!(text.contains("unit-tests"), "must name the offending id: {text}");
    assert!(text.contains("\"unit tests\""), "must list the known ids: {text}");
}

#[test]
fn an_empty_command_is_rejected_rather_than_run() {
    let repo = tempfile::tempdir().unwrap();
    write_config(repo.path(), "[gates.\"unit tests\"]\ncommand = []\n");
    let text = err(repo.path(), "an empty argv must be refused").to_string();
    assert!(text.contains("empty command"), "got {text}");
}

#[test]
fn an_unknown_key_is_rejected_and_names_the_file() {
    let repo = tempfile::tempdir().unwrap();
    write_config(repo.path(), "[gates.\"unit tests\"]\ncmd = [\"npm\"]\n");
    let text = err(repo.path(), "deny_unknown_fields must reject `cmd`").to_string();
    assert!(text.contains("gates.toml"), "must name the file: {text}");
}
