use super::*;

// --- notice() unit tests: all three branches, no process-global side-effects ---

#[test]
fn notice_none_path_returns_install_hint() {
    let current = Path::new("/some/binary/fleet");
    let msg = notice(current, None, false).expect("should return Some when fleet not in PATH");
    assert!(msg.contains("not found in PATH"), "got: {msg}");
    assert!(msg.contains("install.sh"), "got: {msg}");
}

#[test]
fn notice_same_path_returns_none() {
    let path = Path::new("/same/fleet");
    assert!(notice(path, Some(path), false).is_none());
}

#[test]
fn notice_different_path_returns_mismatch_hint() {
    let current = Path::new("/new/fleet");
    let path_fleet = Path::new("/old/fleet");
    let msg =
        notice(current, Some(path_fleet), false).expect("should return Some when paths differ");
    assert!(msg.contains("/old/fleet"), "got: {msg}");
    assert!(msg.contains("/new/fleet"), "got: {msg}");
    assert!(msg.contains("install.sh"), "got: {msg}");
}

// --- find_in_path / is_executable helper tests ---

#[test]
fn find_in_path_finds_existing_executable() {
    let found = find_in_path("git");
    assert!(found.is_some(), "expected to find git in PATH");
    assert!(found.unwrap().exists());
}

#[test]
fn find_in_path_returns_none_for_nonexistent_name() {
    assert!(find_in_path("__fleet_test_does_not_exist_xyz").is_none());
}

#[test]
fn is_executable_true_for_real_binary() {
    let git = find_in_path("git").expect("git must exist");
    assert!(is_executable(&git));
}

#[test]
fn is_executable_false_for_nonexistent_path() {
    assert!(!is_executable(std::path::Path::new("/does/not/exist")));
}

#[test]
fn is_executable_false_for_non_executable_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("not_exec");
    std::fs::write(&f, b"hello").unwrap();
    let mut perms = std::fs::metadata(&f).unwrap().permissions();
    perms.set_mode(0o644);
    std::fs::set_permissions(&f, perms).unwrap();
    assert!(!is_executable(&f));
}
