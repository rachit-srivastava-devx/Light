//! `fleet graph` used to walk every directory unconditionally, so a repo carrying a heavy
//! `target/`/`.git/`-shaped subtree (build artifacts, VCS objects) pegged a core forever with no
//! output and no timeout. This drives the real binary against a scratch repo whose `target/` and
//! `.git/` dirs are stuffed with thousands of files and asserts the command still finishes
//! quickly -- proving the skip-list actually prunes those dirs rather than merely being present
//! in the source.

use std::fs;
use std::time::{Duration, Instant};

mod support;
use support::cmd;

/// Fills `dir` with `n` throwaway `.rs` files one dir level deep, so a naive walker that does not
/// skip `dir` pays real IO/parse cost proportional to `n`.
fn stuff(dir: &std::path::Path, n: usize) {
    fs::create_dir_all(dir).unwrap();
    for i in 0..n {
        fs::write(dir.join(format!("f{i}.rs")), "fn x() {}\n").unwrap();
    }
}

#[test]
fn graph_finishes_quickly_despite_a_heavy_target_and_git_subtree() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("lib.rs"), "fn real() {}\n").unwrap();
    // Directories a real repo's `fleet graph --repo .` would otherwise hang inside.
    stuff(&root.path().join("target").join("debug").join("deps"), 3000);
    stuff(&root.path().join(".git").join("objects"), 3000);
    stuff(&root.path().join("node_modules").join("pkg"), 1000);

    let started = Instant::now();
    let out = cmd().args(["graph", "--repo", root.path().to_str().unwrap()]).output().expect("binary runs");
    let elapsed = started.elapsed();

    assert!(out.status.success(), "graph failed: {}", String::from_utf8_lossy(&out.stderr));
    assert!(elapsed < Duration::from_secs(20), "graph took {elapsed:?}, should skip target/.git/node_modules");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("files_scanned: 1"), "expected only the one real file, got: {stdout}");
}
