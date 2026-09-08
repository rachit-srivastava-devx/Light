use std::env;
use std::io::Write;
use std::os::unix::io::FromRawFd;
use std::os::unix::net::UnixStream;

fn send(bytes: &[u8]) {
    let mut sock = unsafe { UnixStream::from_raw_fd(3) };
    let _ = sock.write_all(bytes);
}

pub fn send_done() {
    let packet = br#"{"schema_version":"1.0","kind":"done","body":{"ok":true},"resolved_model":"stub-1","tokens":42}"#;
    send(packet);
}

/// Same `done` packet as `send_done`, but first writes a real file into `worktree` -- the
/// regression fixture for the "worker actually did something" happy path (`change_detect`
/// must NOT downgrade this one).
pub fn send_done_with_change(worktree: &str) {
    let _ = std::fs::write(format!("{worktree}/fixture-change.txt"), "real change\n");
    send_done();
}

pub fn send_refuse() {
    let packet = br#"{"schema_version":"1.0","kind":"refuse","body":{},"reason":"fixture refusal"}"#;
    send(packet);
}

pub fn send_malformed() {
    send(b"not-json-at-all");
}

pub fn exit_silently() {}

/// Reports HOME/XDG_CONFIG_HOME plus two ambient-leak signals that HOME/XDG alone cannot prove:
/// whether a non-allowlisted sentinel var the caller set in ITS OWN env is still visible here
/// (it must not be -- that is what `env_clear()` is for), and the total var count (a hermetic
/// child sees at most 6: PATH/HOME/XDG_CONFIG_HOME/XDG_DATA_HOME/XDG_CACHE_HOME/LANG, so a much
/// larger count is itself evidence the parent's real environment leaked through).
pub fn send_env_dump(worktree: &str) {
    let _ = std::fs::write(format!("{worktree}/fixture-change.txt"), "real change\n");
    let home = env::var("HOME").unwrap_or_default();
    let xdg = env::var("XDG_CONFIG_HOME").unwrap_or_default();
    let sentinel_leaked = env::var("FLEET_WORKER_TEST_SENTINEL_LEAK").is_ok();
    let var_count = env::vars().count();
    let body = format!(
        r#"{{"home":{home:?},"xdg_config_home":{xdg:?},"sentinel_leaked":{sentinel_leaked},"var_count":{var_count}}}"#
    );
    let packet = format!(r#"{{"schema_version":"1.0","kind":"done","body":{body}}}"#);
    send(packet.as_bytes());
}

/// Forks a long-sleeping grandchild that records its own pid under `<worktree>/.grandchild-pid`,
/// then the fixture itself sleeps past any reasonable deadline without ever touching fd-3 --
/// this is the regression fixture for `terminate_group`'s whole-process-group kill.
pub fn hang_with_grandchild(args: &[String]) {
    let worktree = args.get(3).cloned().unwrap_or_default();
    let marker = format!("{worktree}/.grandchild-pid").replace('\'', "'\\''");
    let script = format!("echo $$ > '{marker}'; sleep 300");
    let _ = std::process::Command::new("sh").arg("-c").arg(script).spawn();
    std::thread::sleep(std::time::Duration::from_secs(300));
}
