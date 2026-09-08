//! `FileCursorStore`: absent-file vs. corrupt-file are distinct outcomes (`Ok(None)` vs. `Err`),
//! and a save/load roundtrip is durable across a fresh handle to the same directory.

use fleet_stream::{CursorStore, FileCursorStore};

#[test]
fn absent_cursor_file_is_ok_none_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileCursorStore::new(dir.path().join("cursors"));
    assert_eq!(store.load("file").unwrap(), None);
}

#[test]
fn save_then_load_roundtrips_across_a_fresh_handle() {
    let dir = tempfile::tempdir().unwrap();
    let cursor_dir = dir.path().join("cursors");
    let store = FileCursorStore::new(&cursor_dir);
    store.save("file", 42).unwrap();
    store.save("file", 43).unwrap();

    let reopened = FileCursorStore::new(&cursor_dir);
    assert_eq!(reopened.load("file").unwrap(), Some(43));
}

#[test]
fn distinct_sink_ids_get_distinct_cursors() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileCursorStore::new(dir.path().join("cursors"));
    store.save("file", 1).unwrap();
    store.save("orb", 99).unwrap();
    assert_eq!(store.load("file").unwrap(), Some(1));
    assert_eq!(store.load("orb").unwrap(), Some(99));
}

#[test]
fn a_cursor_file_that_exists_but_does_not_parse_is_a_typed_error_not_none() {
    let dir = tempfile::tempdir().unwrap();
    let cursor_dir = dir.path().join("cursors");
    std::fs::create_dir_all(&cursor_dir).unwrap();
    std::fs::write(cursor_dir.join("file.cursor"), b"not-a-number").unwrap();

    let store = FileCursorStore::new(&cursor_dir);
    assert!(store.load("file").is_err(), "corrupt cursor file must not be conflated with 'absent'");
}

/// The atomic-rename mechanism (write to a temp file, then rename into place) always replaces
/// the cursor file's whole content in one shot. A save that instead wrote in place, in-flight,
/// without atomically replacing the whole file could leave trailing bytes from a longer previous
/// value behind a shorter new one. This asserts the actual observable contract -- a shorter value
/// fully replaces a longer one -- rather than the mechanism by name, so a regression to a
/// non-atomic in-place write trips it.
#[test]
fn a_shorter_saved_value_fully_replaces_a_longer_previous_one() {
    let dir = tempfile::tempdir().unwrap();
    let cursor_dir = dir.path().join("cursors");
    let store = FileCursorStore::new(&cursor_dir);
    store.save("file", 999_999_999).unwrap();
    store.save("file", 5).unwrap();
    assert_eq!(store.load("file").unwrap(), Some(5), "no trailing bytes from the longer prior write survive");
}
