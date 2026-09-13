//! Builds the rayon thread pool sized by `ConcurrencyCap` for the pipeline's CPU-bound stages
//! (`Scan`, parts of `Verify`). A scoped `ThreadPool` value, not the global pool -- so building
//! one in a test never collides with another test's global-pool init.

use super::concurrency_cap::ConcurrencyCap;

pub fn build(cap: ConcurrencyCap) -> Result<rayon::ThreadPool, rayon::ThreadPoolBuildError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(cap.get())
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_pool_that_runs_work() {
        let pool = build(ConcurrencyCap::compute(4, 4, 4)).expect("pool builds");
        let sum: i32 = pool.install(|| (1..=4).sum());
        assert_eq!(sum, 10);
    }
}
