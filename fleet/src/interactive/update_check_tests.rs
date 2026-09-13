use super::*;

#[test]
fn sha_in_remote_finds_sha_on_feature_branch() {
    // Local built from dev which is 3 commits ahead of main — ls-remote still shows dev ref.
    let output = concat!(
        "175fb18284d0abc123def456\trefs/heads/dev\n",
        "2441646abaed1234567890ab\trefs/heads/main\n",
        "2441646abaed1234567890ab\tHEAD\n",
    );
    assert!(
        sha_in_remote("175fb18284d0", output),
        "dev sha must be found via refs/heads/dev"
    );
}

#[test]
fn sha_in_remote_returns_false_when_commit_is_unpublished() {
    // An unpublished local commit is not evidence that a newer binary exists.
    let output = concat!(
        "2441646abaed1234567890ab\trefs/heads/main\n",
        "2441646abaed1234567890ab\tHEAD\n",
    );
    assert!(!sha_in_remote("175fb18284d0", output));
}

#[test]
fn sha_in_remote_handles_empty_output() {
    assert!(!sha_in_remote("175fb18284d0", ""));
}

#[test]
fn sha_in_remote_matches_main_branch_commit() {
    // When built from main at the same commit as origin/main.
    let local = "2441646abaed";
    let output = concat!(
        "2441646abaed1234567890ab\trefs/heads/main\n",
        "2441646abaed1234567890ab\tHEAD\n",
    );
    assert!(sha_in_remote(local, output));
}

#[test]
fn sha_in_remote_rejects_short_remote_sha() {
    // A remote SHA shorter than SHA_LEN should not match anything.
    let output = "abc\tHEAD\n";
    assert!(!sha_in_remote("abc123def456", output));
}
