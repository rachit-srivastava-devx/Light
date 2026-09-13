//! Unit tests for slash command dispatching in the interactive DX.

#[cfg(test)]
mod tests {
    use crate::interactive::commands::{execute_slash, CommandOutcome};
    use crate::runtime::ConcurrencyCap;

    #[test]
    fn slash_exit_and_quit_return_exit() {
        let cap = ConcurrencyCap::minimum();
        assert!(matches!(
            execute_slash("/exit", cap, "m", false),
            CommandOutcome::Exit
        ));
        assert!(matches!(
            execute_slash("/quit", cap, "m", false),
            CommandOutcome::Exit
        ));
    }

    #[test]
    fn slash_mode_returns_toggle() {
        let cap = ConcurrencyCap::minimum();
        assert!(matches!(
            execute_slash("/mode", cap, "m", false),
            CommandOutcome::ToggleMode
        ));
    }

    #[test]
    fn slash_clear_returns_clear() {
        let cap = ConcurrencyCap::minimum();
        assert!(matches!(
            execute_slash("/clear", cap, "m", false),
            CommandOutcome::Clear
        ));
    }

    #[test]
    fn slash_model_switches_model() {
        let cap = ConcurrencyCap::minimum();
        let out = execute_slash("/model gpt-4o", cap, "claude-3-7-sonnet", false);
        match out {
            CommandOutcome::SetModel(m) => assert_eq!(m, "gpt-4o"),
            _ => panic!("expected SetModel"),
        }
    }

    #[test]
    fn slash_help_and_status_continue() {
        let cap = ConcurrencyCap::minimum();
        assert!(matches!(
            execute_slash("/help", cap, "m", false),
            CommandOutcome::Continue
        ));
        assert!(matches!(
            execute_slash("/status", cap, "m", false),
            CommandOutcome::Continue
        ));
        assert!(matches!(
            execute_slash("/gates", cap, "m", false),
            CommandOutcome::Continue
        ));
    }
}
