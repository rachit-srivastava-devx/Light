//! Read-only, bounded consumer for the external Claude-history golden dataset.
use serde::Deserialize;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use verify::{
    CanonicalGateResult, CanonicalGateSpec, ReviewedCandidate, Status, assemble_gate_evidence,
};

#[derive(Deserialize)]
struct Manifest {
    root: String,
    max_bytes_per_file: usize,
    max_records_per_file: usize,
    files: Vec<FileEntry>,
    expected: Expected,
}
#[derive(Deserialize)]
struct FileEntry {
    path: String,
    bytes: usize,
    records: usize,
}
#[derive(Deserialize)]
struct Expected {
    files: usize,
    records: usize,
    bytes: usize,
    blake3: String,
}

fn read_prefix(path: &Path, cap: usize) -> (Vec<u8>, usize) {
    let file = File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut reader = BufReader::new(file).take(cap as u64);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    let records = bytes.iter().filter(|byte| **byte == b'\n').count();
    (bytes, records)
}

#[path = "external_acceptance_tests.rs"]
mod acceptance;
#[path = "external_hash_tests.rs"]
mod hash;
