fn main() {
    // Derive the path to the fleet binary from OUT_DIR so that
    // env!("CARGO_BIN_EXE_fleet") compiles in tests/real_binary.rs.
    // OUT_DIR = {target}/{profile}/build/user-cli-{hash}/out
    // fleet binary = {target}/{profile}/fleet
    let out = std::env::var("OUT_DIR").unwrap();
    let profile_dir = std::path::Path::new(&out)
        .parent() // build/user-cli-{hash}
        .and_then(|p| p.parent()) // build/
        .and_then(|p| p.parent()) // {profile}/
        .expect("unexpected OUT_DIR structure");
    let fleet_bin = profile_dir.join("fleet");
    println!("cargo:rustc-env=CARGO_BIN_EXE_fleet={}", fleet_bin.display());
}
