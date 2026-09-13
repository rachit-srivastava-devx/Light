use super::AgentToolPolicy;
use std::process::Command;

fn args(policy: AgentToolPolicy) -> Vec<String> {
    let mut command = Command::new("claude");
    policy.configure_claude(&mut command);
    command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

fn tools_arg<'a>(args: &'a [String]) -> &'a str {
    args.iter()
        .find_map(|a| a.strip_prefix("--tools="))
        .expect("--tools= present")
}

#[test]
fn model_only_has_read_tools_and_no_shell_or_writes() {
    let args = args(AgentToolPolicy::approved(false, false));
    let tools = args.last().expect("allowed tools");
    assert_eq!(tools, "--allowedTools=Read,Glob,Grep");
    assert!(!tools.contains("Bash"));
    assert!(!tools.contains("Write"));
    assert!(args.contains(&"--restricted".into()));
}

#[test]
fn approved_commands_do_not_include_publish_capabilities() {
    let args = args(AgentToolPolicy::approved(true, true));
    let tools = args.last().expect("allowed tools");
    assert!(tools.contains("Edit,Write"));
    assert!(tools.contains("Bash(cargo test *)"));
    for forbidden in ["git push", "gh ", "curl", "ssh"] {
        assert!(
            !tools.contains(forbidden),
            "unexpected {forbidden}: {tools}"
        );
    }
}

/// Regression: `--restricted` only re-enables a tool it removed (Bash included) when `--tools`
/// names it bare. A pattern-qualified entry there (what `--allowedTools` needs) leaves Bash
/// off entirely -- every command-effect task silently degraded to read-only, verified live
/// against the real `claude` binary before this fix.
#[test]
fn command_policy_enables_bare_bash_in_tools_not_just_allowed_tools() {
    let args = args(AgentToolPolicy::approved(false, true));
    let tools = tools_arg(&args);
    assert_eq!(tools, "Read,Glob,Grep,Bash");
    let allowed = args.last().expect("allowed tools");
    assert!(allowed.contains("Bash(git log *)"));
}

#[test]
fn read_only_policy_never_names_bash_in_either_tools_flag() {
    let args = args(AgentToolPolicy::approved(false, false));
    assert_eq!(tools_arg(&args), "Read,Glob,Grep");
}
