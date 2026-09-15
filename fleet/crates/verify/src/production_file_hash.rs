use super::super::acceptance::spec_hash::field;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub(crate) fn bounded_file_digest(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let limit = 1_048_576u64;
    let mut bytes = Vec::new();
    file.by_ref().take(limit).read_to_end(&mut bytes)?;
    let mut hasher = blake3::Hasher::new();
    field(&mut hasher, b"fleet.file.v1");
    field(&mut hasher, &size.to_le_bytes());
    field(&mut hasher, &bytes);
    field(
        &mut hasher,
        if size > limit {
            &b"truncated"[..]
        } else {
            &b"complete"[..]
        },
    );
    Ok(hasher.finalize().as_bytes().to_vec())
}
