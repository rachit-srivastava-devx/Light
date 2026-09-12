//! fd-3 protocol, child (send) side: detecting the channel is really wired, and serializing the
//! receipt packets the parent's `interpret_fd3` reads back. `resolved_model`/`tokens`/`reason`
//! are TOP-LEVEL packet fields, not nested under `body` -- `interpret_fd3` and the fixture in
//! `tests/fixtures/fixture_scenarios.rs` both already read them that way; this is that contract's
//! only write side, so it must match exactly rather than invent a second shape.

use serde_json::{json, Value};
use std::os::raw::c_int;

/// Whether the child's own fd 3 is actually the wire channel the parent wired (a socket -- see
/// `fd3::wire`), not merely SOME open fd that happens to occupy slot 3 (the tokio runtime this
/// binary builds opens its own low-numbered fds, e.g. a kqueue, so `fcntl(3, F_GETFD) != -1`
/// alone is a proxy, not the property -- PRINCIPLES). `getsockopt` fails with `ENOTSOCK` on
/// anything that isn't a socket, the actual distinguishing fact. `false` when `__agent` is run
/// directly from a shell instead of spawned by `fleet-worker`'s `spawn`.
pub fn child_channel_open() -> bool {
    let mut sock_type: c_int = 0;
    let mut len = std::mem::size_of::<c_int>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            3,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            &mut sock_type as *mut c_int as *mut libc::c_void,
            &mut len,
        )
    };
    rc == 0
}

pub fn send_done(body: Value, resolved_model: Option<&str>, tokens: Option<u64>) -> Result<(), std::io::Error> {
    let mut packet = json!({"schema_version": "1.0", "kind": "done", "body": body});
    if let Some(m) = resolved_model {
        packet["resolved_model"] = json!(m);
    }
    if let Some(t) = tokens {
        packet["tokens"] = json!(t);
    }
    send_raw(&packet)
}

pub fn send_refuse(reason: &str, body: Value) -> Result<(), std::io::Error> {
    send_raw(&json!({"schema_version": "1.0", "kind": "refuse", "body": body, "reason": reason}))
}

fn send_raw(packet: &Value) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec(packet)?;
    let sent = unsafe { libc::send(3, bytes.as_ptr() as *const libc::c_void, bytes.len(), 0) };
    if sent != bytes.len() as isize {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
