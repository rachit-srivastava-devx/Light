//! fd-3 receive + submission validation, ported verbatim from `main.rs::spawn_agent_with_args`'s
//! recv half and `main.rs::validate_submission`.

use std::os::raw::c_int;

/// Receive up to 65536 bytes off `parent_fd`. `Some(len == buf.len())` signals the packet may
/// have been truncated at the buffer ceiling -- a proxy, not proof, of an exact fit (PRINCIPLES
/// "a proxy is not the property").
pub fn recv(parent_fd: c_int) -> Option<(Vec<u8>, bool)> {
    let mut packet = vec![0u8; 65536];
    let received = unsafe {
        libc::recv(parent_fd, packet.as_mut_ptr() as *mut libc::c_void, packet.len(), 0)
    };
    unsafe {
        libc::close(parent_fd);
    }
    if received < 0 {
        return None;
    }
    let at_ceiling = received as usize == packet.len();
    packet.truncate(received as usize);
    Some((packet, at_ceiling))
}

/// Checks `schema_version=="1.0"`, `kind` in `{note,done,refuse}`, `body` is an object.
pub fn validate_submission(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else { return false };
    if object.get("schema_version") != Some(&serde_json::Value::String("1.0".to_string())) {
        return false;
    }
    if !matches!(
        object.get("kind").and_then(serde_json::Value::as_str),
        Some("note") | Some("done") | Some("refuse")
    ) {
        return false;
    }
    object.get("body").is_some_and(serde_json::Value::is_object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_submission_rejects_wrong_schema_version() {
        let value = json!({"schema_version": "2.0", "kind": "done", "body": {}});
        assert!(!validate_submission(&value));
    }

    #[test]
    fn validate_submission_rejects_unknown_kind() {
        let value = json!({"schema_version": "1.0", "kind": "unexpected", "body": {}});
        assert!(!validate_submission(&value));
    }

    #[test]
    fn validate_submission_accepts_a_well_formed_done_packet() {
        let value = json!({"schema_version": "1.0", "kind": "done", "body": {}});
        assert!(validate_submission(&value));
    }
}
