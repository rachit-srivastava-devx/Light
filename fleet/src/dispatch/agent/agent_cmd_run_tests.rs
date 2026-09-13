use super::{claude_stream_args, failure_detail, run_cli, AgentOutcome};
use builder::CliAdapter;
use std::path::Path;

const LOCAL_ONLY_GREETING_TASK: &str =
    "Add a pure helper that formats a greeting, test it, and do not publish or push anything.";

#[test]
fn local_only_user_prompt_refuses_before_an_unconfigured_agent_can_claim_work() {
    let result = run_cli(
        CliAdapter::Freelane,
        Path::new("/"),
        LOCAL_ONLY_GREETING_TASK,
        None,
    );
    match result {
        AgentOutcome::Refused(reason) => {
            assert_eq!(reason, "freelane has no direct CLI executable");
        }
        AgentOutcome::Done { .. } => panic!("an adapter without a binary must never report done"),
    }
}

#[test]
fn stdout_only_provider_refusal_is_preserved() {
    assert_eq!(
        failure_detail(b"provider says wait", b""),
        "provider says wait"
    );
}

#[test]
fn claude_streaming_requires_verbose_batch_mode() {
    assert_eq!(
        claude_stream_args(),
        [
            "--print",
            "--verbose",
            "--output-format",
            "stream-json",
            "--include-partial-messages",
        ]
    );
}
