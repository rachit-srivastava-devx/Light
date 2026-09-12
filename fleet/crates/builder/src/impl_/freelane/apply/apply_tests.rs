use super::*;
use std::fs;

fn worktree() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

#[test]
fn single_fence_with_clear_target_is_applied() {
    let wt = worktree();
    let reply = "```path:src/foo.rs\nfn main() {}\n```\n";
    let applied = apply(wt.path(), reply).expect("should apply");
    // `apply` returns CANONICAL paths (that resolution IS the containment guard), so must we: on macOS `/var` symlinks to `/private/var`.
    let expected = wt.path().canonicalize().unwrap().join("src/foo.rs");
    assert_eq!(applied, vec![expected]);
    assert_eq!(fs::read_to_string(wt.path().join("src/foo.rs")).unwrap(), "fn main() {}\n");
}

#[test]
fn no_fence_at_all_is_refused() {
    let wt = worktree();
    let err = apply(wt.path(), "just prose, no code here at all").unwrap_err();
    assert!(matches!(err, ApplyError::NoFence), "{err:?}");
}

#[test]
fn fence_with_no_target_is_refused_naming_the_ambiguity() {
    let wt = worktree();
    let reply = "```rust\nfn main() {}\n```\n";
    let err = apply(wt.path(), reply).unwrap_err();
    assert!(matches!(err, ApplyError::AmbiguousTarget { index: 1 }), "{err:?}");
    assert!(err.to_string().contains("fence #1"), "{err}");
}

#[test]
fn absolute_path_is_refused() {
    let wt = worktree();
    let reply = "```path:/etc/passwd\nhacked\n```\n";
    let err = apply(wt.path(), reply).unwrap_err();
    assert!(matches!(err, ApplyError::AbsolutePath { .. }), "{err:?}");
    assert!(!wt.path().join("etc/passwd").exists());
}

#[test]
fn parent_traversal_is_refused_and_nothing_written_outside() {
    let outer = tempfile::tempdir().unwrap();
    let wt_path = outer.path().join("worktree");
    fs::create_dir_all(&wt_path).unwrap();
    let sentinel = outer.path().join("victim.txt");
    fs::write(&sentinel, "untouched").unwrap();

    let reply = "```path:../victim.txt\nhacked\n```\n";
    let err = apply(&wt_path, reply).unwrap_err();
    assert!(matches!(err, ApplyError::PathTraversal { .. }), "{err:?}");
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "untouched", "sentinel must be untouched");
}

#[test]
fn symlinked_target_escaping_the_worktree_is_refused() {
    let outer = tempfile::tempdir().unwrap();
    let wt_path = outer.path().join("worktree");
    fs::create_dir_all(&wt_path).unwrap();
    let outside_dir = outer.path().join("outside");
    fs::create_dir_all(&outside_dir).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_dir, wt_path.join("linked")).unwrap();

    let reply = "```path:linked/evil.rs\nhacked\n```\n";
    let err = apply(&wt_path, reply).unwrap_err();
    assert!(matches!(err, ApplyError::EscapesWorktree { .. }), "{err:?}");
    assert!(!outside_dir.join("evil.rs").exists());
}

#[test]
fn one_bad_file_among_several_applies_nothing() {
    let wt = worktree();
    let reply = "```path:src/good.rs\nfn ok() {}\n```\n```path:/abs/bad.rs\nfn bad() {}\n```\n";
    let err = apply(wt.path(), reply).unwrap_err();
    assert!(matches!(err, ApplyError::AbsolutePath { .. }), "{err:?}");
    assert!(!wt.path().join("src/good.rs").exists(), "nothing must be written on a refused batch");
}
