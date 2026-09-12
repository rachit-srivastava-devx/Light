//! Writes the embedded gate assets to a fresh directory, then walks it once to set the
//! executable bit on every `.sh` file -- `include_dir::Dir::extract` writes plain files only.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::embed::{
    CORPUS_DIR, DETECTOR_INTEGRITY_SH, POLICY_DIR, RECUR_GATE_SH, SEMGREP_GATE_SH, TRIVY_GATE_SH,
};

pub(super) fn materialize(root: &Path) -> io::Result<()> {
    write_script(root, "semgrep-gate.sh", SEMGREP_GATE_SH)?;
    write_script(root, "trivy-gate.sh", TRIVY_GATE_SH)?;
    write_script(root, "recur-gate.sh", RECUR_GATE_SH)?;
    write_script(root, "detector-integrity.sh", DETECTOR_INTEGRITY_SH)?;

    let policy_root = root.join("policy");
    fs::create_dir_all(&policy_root)?;
    POLICY_DIR.extract(&policy_root)?;

    let corpus_root = root.join("corpus");
    fs::create_dir_all(&corpus_root)?;
    CORPUS_DIR.extract(&corpus_root)?;

    mark_shell_scripts_executable(root)
}

fn write_script(root: &Path, name: &str, content: &str) -> io::Result<()> {
    let path = root.join(name);
    fs::write(&path, content)?;
    make_executable(&path)
}

/// Recursively `chmod +x` every `*.sh` file under `dir` -- covers `policy/run.sh` and every
/// `corpus/*.sh` detector, none of which `include_dir::Dir::extract` marks executable on its own.
fn mark_shell_scripts_executable(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            mark_shell_scripts_executable(&path)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("sh") {
            make_executable(&path)?;
        }
    }
    Ok(())
}

fn make_executable(path: &Path) -> io::Result<()> {
    let mut perm = fs::metadata(path)?.permissions();
    perm.set_mode(0o755);
    fs::set_permissions(path, perm)
}
