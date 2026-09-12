use std::process::Command;

fn fleet() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fleet"))
}

#[test]
fn version_exits_zero() {
    let out = fleet().arg("--version").output().expect("fleet binary runs");
    assert!(
        out.status.success(),
        "fleet --version failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn route_json_exits_with_known_code() {
    let out = fleet()
        .args(["route", "--json"])
        .env("FLEET_LOAD_FACTOR", "10000")
        .output()
        .expect("fleet binary runs");
    // route without a valid task context exits 7 (refusal) or 0; either is acceptable
    let code = out.status.code().unwrap_or(1);
    assert!(
        code == 0 || code == 7,
        "unexpected exit code {code} from fleet route --json"
    );
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let out = fleet()
        .arg("__test_zzzz_unknown")
        .output()
        .expect("fleet binary runs");
    assert!(!out.status.success(), "unknown subcommand must fail");
}
