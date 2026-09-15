use super::*;

#[test]
fn external_dataset_is_bounded_and_hashed() {
    let manifest: Manifest = serde_json::from_str(include_str!("golden_manifest.json")).unwrap();
    let root = std::env::var_os("FLEET_VERIFY_GOLDEN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&manifest.root));
    assert!(
        root.is_dir(),
        "external golden dataset is required: {}",
        root.display()
    );
    assert_eq!(manifest.files.len(), manifest.expected.files);
    assert!(manifest.max_bytes_per_file > 0 && manifest.max_records_per_file > 0);
    let mut hasher = blake3::Hasher::new();
    let mut total_bytes = 0;
    let mut total_records = 0;
    for entry in &manifest.files {
        hasher.update(entry.path.as_bytes());
        hasher.update(&[0]);
        let (bytes, records) = read_prefix(&root.join(&entry.path), manifest.max_bytes_per_file);
        assert_eq!(
            bytes.len(),
            entry.bytes,
            "byte cap changed for {}",
            entry.path
        );
        assert_eq!(
            records, entry.records,
            "record cap changed for {}",
            entry.path
        );
        assert!(records <= manifest.max_records_per_file);
        for line in bytes
            .split(|byte| *byte == b'\n')
            .take(records)
            .filter(|line| !line.is_empty())
        {
            let value: serde_json::Value = serde_json::from_slice(line)
                .unwrap_or_else(|e| panic!("invalid bounded JSON in {}: {e}", entry.path));
            assert!(
                value.is_object(),
                "dataset record must be an object: {}",
                entry.path
            );
        }
        hasher.update(&bytes);
        hasher.update(&[0]);
        total_bytes += bytes.len();
        total_records += records;
    }
    assert_eq!(total_bytes, manifest.expected.bytes);
    assert_eq!(total_records, manifest.expected.records);
    assert_eq!(
        format!("blake3:{}", hasher.finalize().to_hex()),
        manifest.expected.blake3
    );
}
