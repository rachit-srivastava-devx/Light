use fleet_ledger::{append_receipt, verify_chain, ReceiptInput};
use proptest::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn ledger_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fleet-ledger-proptest-{label}-{}-{stamp}-{id}.jsonl",
        std::process::id()
    ))
}

fn row<'a>(event: &'a str, component: &'a str, task: &'a str) -> ReceiptInput<'a> {
    ReceiptInput {
        ts: "2026-01-01T00:00:00Z",
        event,
        component,
        task_id: task,
        generator_model: "sonnet",
        verifier_model: "opus",
        exit_code: "0",
        tokens_in: "1",
        tokens_out: "2",
        cost_micro_usd: "3",
    }
}

fn remove(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}.ckpt", path.display()));
    let _ = fs::remove_dir_all(format!("{}.lock", path.display()));
}

proptest! {
    #[test]
    fn any_sequence_of_appends_verifies(events in prop::collection::vec("[a-zA-Z0-9_-]{1,12}", 1..8)) {
        let path = ledger_path("sequence");
        for event in &events {
            append_receipt(&path, row(event, "component", "task")).unwrap();
        }
        prop_assert_eq!(verify_chain(&path, false, None).unwrap().lines, events.len());
        remove(&path);
    }

    #[test]
    fn changing_one_hashed_byte_breaks_verification(events in prop::collection::vec("[a-zA-Z0-9_-]{1,12}", 1..8), line_index in 0usize..8, hash_index in 0usize..64) {
        let path = ledger_path("mutation");
        for event in &events {
            append_receipt(&path, row(event, "component", "task")).unwrap();
        }
        let original = fs::read_to_string(&path).unwrap();
        let mut lines: Vec<String> = original.lines().map(ToOwned::to_owned).collect();
        let selected = line_index % lines.len();
        let marker = "\"hash\":\"";
        let start = lines[selected].find(marker).unwrap() + marker.len();
        let offset = start + (hash_index % 64);
        let mut bytes = lines[selected].as_bytes().to_vec();
        let current = bytes[offset];
        bytes[offset] = if current == b'0' { b'1' } else { b'0' };
        lines[selected] = String::from_utf8(bytes).unwrap();
        fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
        prop_assert!(verify_chain(&path, false, None).is_err());
        remove(&path);
    }
}
