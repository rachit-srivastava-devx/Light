//! The lane concurrency cap: `min(cores - 2, ram_lanes, review_cap)`, floored at 1. Computed
//! once at startup (`main.rs`) and threaded down as a plain value to every stage that needs a
//! bound on how many lanes may run at once. Pure, total, never panics -- see BLUEPRINT §3/§6.

use std::num::NonZeroUsize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConcurrencyCap(NonZeroUsize);

impl ConcurrencyCap {
    /// `available_cores` normally comes from `std::thread::available_parallelism()`;
    /// `ram_lanes` is the caller's estimate of how many lanes fit in RAM (`usize::MAX` means
    /// "unconstrained by RAM"); `review_cap` is the hard ceiling on lanes awaiting review at
    /// once (product default: 3). Never zero: any input driving the raw minimum to 0 is caught
    /// by the trailing `.max(1)`.
    pub fn compute(available_cores: usize, ram_lanes: usize, review_cap: usize) -> Self {
        let cap = available_cores
            .saturating_sub(2)
            .min(ram_lanes)
            .min(review_cap)
            .max(1);
        Self(NonZeroUsize::new(cap).expect("max(1) guarantees nonzero"))
    }

    /// Reads the real environment: `std::thread::available_parallelism()` (falls back to 1 on
    /// error) for cores, and the caller-supplied `ram_lanes`/`review_cap`. This is the one call
    /// site allowed to touch `available_parallelism` -- everywhere else takes the cap as data.
    pub fn from_env(ram_lanes: usize, review_cap: usize) -> Self {
        let cores = std::thread::available_parallelism().map(NonZeroUsize::get).unwrap_or(1);
        Self::compute(cores, ram_lanes, review_cap)
    }

    pub fn get(self) -> usize {
        self.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_is_never_zero_even_when_every_input_forces_zero() {
        assert_eq!(ConcurrencyCap::compute(2, 0, 0).get(), 1);
        assert_eq!(ConcurrencyCap::compute(0, 5, 5).get(), 1);
        assert_eq!(ConcurrencyCap::compute(5, 5, 0).get(), 1);
    }

    #[test]
    fn cap_is_the_strict_minimum_of_the_three_inputs() {
        let table = [
            ((10usize, 3usize, 5usize), 3usize), // ram binds
            ((10, 100, 2), 2),                   // review binds
            ((6, 100, 100), 4),                  // cores-2 binds
        ];
        for ((cores, ram, review), expected) in table {
            assert_eq!(ConcurrencyCap::compute(cores, ram, review).get(), expected);
        }
    }

    #[test]
    fn cap_saturates_instead_of_underflowing_on_tiny_core_counts() {
        for cores in [0usize, 1, 2] {
            assert!(ConcurrencyCap::compute(cores, 16, 16).get() >= 1);
        }
    }

    #[test]
    fn cap_never_panics_on_huge_inputs() {
        assert!(ConcurrencyCap::compute(usize::MAX, usize::MAX, 3).get() >= 1);
    }
}
