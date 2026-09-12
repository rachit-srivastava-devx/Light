//! Writes the embedded freelane assets to a fresh directory and marks `freelane.sh` executable
//! -- `fs::write`ing a `&str` produces a plain file with no exec bit, same as
//! `crates/fleet-verify/src/gates/materialize.rs`.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::embed::{FREELANE_SH, LANES_CONF};

pub(super) fn materialize(root: &Path) -> io::Result<()> {
    let script_path = root.join("freelane.sh");
    fs::write(&script_path, FREELANE_SH)?;
    let mut perm = fs::metadata(&script_path)?.permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&script_path, perm)?;

    // freelane.sh's own default lane_config is `$(dirname "$0")/lanes.conf` -- sit it right next
    // to the materialized script so that default resolves without FREELANE_CONFIG being set.
    fs::write(root.join("lanes.conf"), LANES_CONF)
}
