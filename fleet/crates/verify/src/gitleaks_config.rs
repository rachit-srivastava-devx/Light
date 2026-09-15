use std::env;
use std::time::Duration;

const DEFAULT_BUDGET_SECS: u64 = 60;

pub(super) fn budget() -> Duration {
    let value = env::var("FLEET_GITLEAKS_BUDGET_SECS").ok();
    Duration::from_secs(budget_secs(value.as_deref()))
}

fn budget_secs(value: Option<&str>) -> u64 {
    value
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(DEFAULT_BUDGET_SECS)
}

#[cfg(test)]
mod tests {
    use super::budget_secs;

    #[test]
    fn configured_budget_accepts_integer_seconds() {
        assert_eq!(budget_secs(Some("7")), 7);
    }

    #[test]
    fn malformed_budget_uses_safe_default() {
        assert_eq!(budget_secs(Some("not-a-duration")), 60);
        assert_eq!(budget_secs(None), 60);
    }
}
