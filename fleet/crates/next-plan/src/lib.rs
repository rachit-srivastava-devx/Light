//! `next-plan` — N+1 plan proposal with deduplication and overlap detection.
mod overlap;
mod queue;
mod types;
pub use overlap::propose_next;
pub use queue::{emit_signal, MemQueue, NextQueue};
pub use types::{NextError, NextInput, NextPlanSignal, NextProposal, PlanDraft};

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(d: &str) -> NextPlanSignal {
        NextPlanSignal {
            parent_digest: d.into(),
            candidate: PlanDraft { modules: vec![], digest: d.into() },
            write_set: vec![],
            measure_set: vec![],
        }
    }

    #[test]
    fn n1_plan_starts_while_n_builds() {
        let mut q = MemQueue::new(4);
        emit_signal(sig("digest-n"), &mut q).unwrap();
        assert_eq!(q.depth(), 1);
    }

    #[test]
    fn signal_without_prior_plan_is_noop() {
        let mut q = MemQueue::new(4);
        emit_signal(sig(""), &mut q).unwrap();
        assert_eq!(q.depth(), 0);
    }

    #[test]
    fn duplicate_signal_deduplicated() {
        let mut q = MemQueue::new(4);
        emit_signal(sig("same"), &mut q).unwrap();
        emit_signal(sig("same"), &mut q).unwrap();
        assert_eq!(q.depth(), 1);
    }
}
