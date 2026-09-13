use super::*;

#[test]
fn searched_names_path_and_the_rustup_locations() {
    let text = searched();
    assert!(text.starts_with("$PATH"), "got {text}");
    assert!(
        text.contains(".cargo/bin") || std::env::var_os("HOME").is_none(),
        "got {text}"
    );
}

/// A resolved `GateCommand::Script` path must never be rewritten by the bare-name lookup.
#[test]
fn a_path_bearing_argv0_is_never_rewritten() {
    assert_eq!(resolve_bin("/opt/gates/run.sh"), "/opt/gates/run.sh");
}
