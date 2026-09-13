//! Real-binary smoke test: invoke the `fleet` worker binary with a fixture.
//!
//! Marked `#[ignore]` until `fleet build --fixture <path> --json` is implemented
//! in the fleet-cli crate. The test documents the intended invocation contract:
//! output must contain `checked=N,total=N` with N > 0 and a candidate digest.

#[test]
#[ignore = "fleet build --fixture subcommand not yet implemented in fleet-cli"]
fn fleet_build_fixture_smoke() {
    // Intended invocation once fleet-cli supports the subcommand:
    //
    //   let bin = env!("CARGO_BIN_EXE_fleet");
    //   let out = std::process::Command::new(bin)
    //       .args(["build", "--fixture",
    //              "tests/fixtures/blueprint-builder/scope-limited.json",
    //              "--json"])
    //       .output()
    //       .expect("fleet binary ran");
    //   let stdout = String::from_utf8_lossy(&out.stdout);
    //   assert!(stdout.contains("checked="), "output: {stdout}");
    //   assert!(out.status.success(), "exit: {}", out.status);
}
