//! Voice latency SLOs, quoted verbatim from `docs/BUILD-DIGEST.md` §3. Pure, no I/O — the
//! Observability plane (Tier 7) reports against these; this module only defines and classifies
//! them, so classification is independently unit-testable without a live socket.

/// Deterministic turn (advance/pause/re-anchor, no LLM on the path) — p50 target, ms.
pub const DETERMINISTIC_TURN_P50_MS: u64 = 250;
/// Deterministic turn p99, ms.
pub const DETERMINISTIC_TURN_P99_MS: u64 = 450;
/// Conversational turn (question/chitchat/atomize dispatch) — p50 target, ms.
pub const CONVERSATIONAL_TURN_P50_MS: u64 = 600;
/// Conversational turn p99, ms.
pub const CONVERSATIONAL_TURN_P99_MS: u64 = 1_200;
/// Stop-to-presence cover: the already-running ambient bed must reflect thinking within this budget.
pub const THINK_TIME_COVER_BUDGET_MS: u64 = 250;
/// App-open to the first actually-rendered greeting audio, measured on the device clock.
pub const FIRST_AUDIO_BUDGET_MS: u64 = 250;
/// Barge-in: TTS must yield within this budget once real speech (not a backchannel) is detected.
pub const BARGE_IN_YIELD_BUDGET_MS: u64 = 100;
/// Minimum speech duration to distinguish a real barge-in from a backchannel ("mm-hmm").
pub const BARGE_IN_MIN_SPEECH_MS: u64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotPathBudget {
    FirstAudio,
    ThinkTimeCover,
    BargeInYield,
    BargeInSpeechQualification,
}

/// One lookup owns every live audio deadline. Callers classify observations through this
/// function instead of copying numeric literals into logging or gate code.
pub const fn hot_path_budget_ms(budget: HotPathBudget) -> u64 {
    match budget {
        HotPathBudget::FirstAudio => FIRST_AUDIO_BUDGET_MS,
        HotPathBudget::ThinkTimeCover => THINK_TIME_COVER_BUDGET_MS,
        HotPathBudget::BargeInYield => BARGE_IN_YIELD_BUDGET_MS,
        HotPathBudget::BargeInSpeechQualification => BARGE_IN_MIN_SPEECH_MS,
    }
}

pub const fn hot_path_within_budget(budget: HotPathBudget, observed_ms: u64) -> bool {
    observed_ms <= hot_path_budget_ms(budget)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnKind {
    Deterministic,
    Conversational,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetVerdict {
    WithinP50,
    WithinP99,
    Breached,
}

/// Classifies an observed hop latency against the turn kind's SLO. Pure function — the caller
/// supplies the elapsed time; this file never reads a clock (AGENTS.md invariant 6's spirit
/// extended to Rust: no wall-clock inside pure classification logic).
pub fn classify(turn: TurnKind, elapsed_ms: u64) -> BudgetVerdict {
    let (p50, p99) = match turn {
        TurnKind::Deterministic => (DETERMINISTIC_TURN_P50_MS, DETERMINISTIC_TURN_P99_MS),
        TurnKind::Conversational => (CONVERSATIONAL_TURN_P50_MS, CONVERSATIONAL_TURN_P99_MS),
    };
    if elapsed_ms <= p50 {
        BudgetVerdict::WithinP50
    } else if elapsed_ms <= p99 {
        BudgetVerdict::WithinP99
    } else {
        BudgetVerdict::Breached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_turn_within_p50() {
        assert_eq!(
            classify(TurnKind::Deterministic, 100),
            BudgetVerdict::WithinP50
        );
    }

    #[test]
    fn deterministic_turn_within_p99_but_over_p50() {
        assert_eq!(
            classify(TurnKind::Deterministic, 300),
            BudgetVerdict::WithinP99
        );
    }

    #[test]
    fn deterministic_turn_breached() {
        assert_eq!(
            classify(TurnKind::Deterministic, 500),
            BudgetVerdict::Breached
        );
    }

    #[test]
    fn conversational_turn_within_p50() {
        assert_eq!(
            classify(TurnKind::Conversational, 600),
            BudgetVerdict::WithinP50
        );
    }

    #[test]
    fn conversational_turn_breached_over_p99() {
        assert_eq!(
            classify(TurnKind::Conversational, 1_201),
            BudgetVerdict::Breached
        );
    }

    #[test]
    fn boundary_values_are_inclusive_of_the_stricter_bucket() {
        // exactly at p50 counts as WithinP50, not WithinP99 — the SLO is "p50 <= target".
        assert_eq!(
            classify(TurnKind::Deterministic, DETERMINISTIC_TURN_P50_MS),
            BudgetVerdict::WithinP50
        );
        assert_eq!(
            classify(TurnKind::Deterministic, DETERMINISTIC_TURN_P99_MS),
            BudgetVerdict::WithinP99
        );
    }

    #[test]
    fn every_hot_path_constant_is_consumed_by_the_budget_lookup() {
        assert_eq!(
            hot_path_budget_ms(HotPathBudget::FirstAudio),
            FIRST_AUDIO_BUDGET_MS
        );
        assert_eq!(
            hot_path_budget_ms(HotPathBudget::ThinkTimeCover),
            THINK_TIME_COVER_BUDGET_MS
        );
        assert_eq!(
            hot_path_budget_ms(HotPathBudget::BargeInYield),
            BARGE_IN_YIELD_BUDGET_MS
        );
        assert_eq!(
            hot_path_budget_ms(HotPathBudget::BargeInSpeechQualification),
            BARGE_IN_MIN_SPEECH_MS
        );
    }

    #[test]
    fn first_audio_and_think_time_fail_above_250_ms() {
        for budget in [HotPathBudget::FirstAudio, HotPathBudget::ThinkTimeCover] {
            assert!(hot_path_within_budget(budget, 250));
            assert!(!hot_path_within_budget(budget, 251));
        }
    }
}
