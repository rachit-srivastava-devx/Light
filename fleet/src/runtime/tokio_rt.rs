//! Builds the multi-thread tokio runtime that hosts the pipeline's IO-bound stages
//! (event ingress, the eventual Restate service host). One call site, `main()`.

use super::concurrency_cap::ConcurrencyCap;

pub fn build(cap: ConcurrencyCap) -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(cap.get())
        .enable_all()
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_runnable_runtime() {
        let cap = ConcurrencyCap::compute(4, 4, 4);
        let rt = build(cap).expect("runtime builds");
        let out = rt.block_on(async { 1 + 1 });
        assert_eq!(out, 2);
    }
}
