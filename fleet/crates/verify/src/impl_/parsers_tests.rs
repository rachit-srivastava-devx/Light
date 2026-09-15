use super::policy;
use crate::impl_::DenominatorResult;

#[test]
fn policy_total_is_passed_plus_failed() {
    assert_eq!(
        policy("-- 3 passed, 2 failed (denominator: 5 policies) --", ""),
        DenominatorResult::Counted(3, 5)
    );
}
