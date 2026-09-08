//! The published denominator -- this crate's core invariant: `total == 0` is refused
//! unconditionally at construction, so a `Verdict::Pass` can never carry an empty measurement.

/// A published numerator/total pair (`caught/total`, `passed/checked`, ...). Deliberately cannot
/// be constructed with `total == 0`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Denominator {
    numerator: u64,
    total: u64,
}

/// `Denominator::new` was asked to construct a `0/0` (or `N/0`) denominator.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("a denominator with total=0 asserts nothing and cannot be published as a Pass")]
pub struct ZeroDenominator;

impl Denominator {
    /// `total == 0` is refused unconditionally -- `numerator == 0` alone is a legitimate "0 of N
    /// passed" failure denominator, only an empty `total` asserts nothing.
    pub fn new(numerator: u64, total: u64) -> Result<Self, ZeroDenominator> {
        if total == 0 {
            return Err(ZeroDenominator);
        }
        Ok(Self { numerator, total })
    }

    pub fn numerator(self) -> u64 {
        self.numerator
    }

    pub fn total(self) -> u64 {
        self.total
    }
}

/// What a gate's stdout parser found, before orchestration applies the zero-total rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DenominatorResult {
    Counted(u64, u64),
    Unparseable,
}
