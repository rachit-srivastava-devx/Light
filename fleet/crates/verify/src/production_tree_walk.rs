use super::MAX_FILES;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub(super) fn collect_files(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) {
    if files.len() >= MAX_FILES {
        return;
    }
    let Ok(mut entries) =
        fs::read_dir(directory).map(|entries| entries.filter_map(Result::ok).collect::<Vec<_>>())
    else {
        return;
    };
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if files.len() >= MAX_FILES {
            break;
        }
        if entry.file_name() == ".git" {
            continue;
        }
        let path = entry.path();
        let Ok(kind) = fs::symlink_metadata(&path) else {
            continue;
        };
        if kind.is_dir() {
            collect_files(root, &path, files);
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, path));
        }
    }
}

pub(super) fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.by_ref().take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}
