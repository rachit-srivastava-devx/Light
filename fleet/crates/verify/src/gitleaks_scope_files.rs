use crate::VerifyError;
use std::fs;
use std::path::Path;

pub(super) fn file_count(repo: &Path) -> Result<u64, VerifyError> {
    if !repo.is_dir() {
        return Err(VerifyError::ScannerScopeUnavailable(
            "directory scan target is not a directory".into(),
        ));
    }
    let mut pending = vec![repo.to_path_buf()];
    let mut total = 0u64;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(scope_error)? {
            let entry = entry.map_err(scope_error)?;
            let file_type = entry.file_type().map_err(scope_error)?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() && entry.file_name() != ".git" {
                pending.push(entry.path());
            } else if file_type.is_file() {
                total = total.checked_add(1).ok_or_else(|| {
                    VerifyError::ScannerScopeUnavailable("file scope count overflow".into())
                })?;
            }
        }
    }
    Ok(total)
}

fn scope_error(error: std::io::Error) -> VerifyError {
    VerifyError::ScannerScopeUnavailable(error.to_string())
}
