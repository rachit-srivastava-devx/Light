//! A one-shot cancellation flag shared between the read loop and the work it spawned.
//!
//! Deliberately tiny and dependency-free (`tokio_util::sync::CancellationToken` does the same job,
//! but this crate's dependency list is itself a contract — docs/adr/0004). Two properties the rest
//! of the relay depends on:
//!
//!   - `is_cancelled` is callable from BLOCKING code. The HTTP provider polls it between socket
//!     reads while it is parked on a synchronous `read`, so it must not require an async context.
//!   - `cancelled()` is a future, so a speech task waiting on a bounded channel wakes the moment a
//!     barge-in lands instead of at the next chunk boundary.
//!
//! Cancellation is monotonic: once set it never clears, so a clone handed to a worker can never
//! observe a cancel and then un-observe it. Cloning is cheap (one `Arc` bump).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;

use crate::provider::CancelSignal;

#[derive(Clone, Default)]
pub struct CancelFlag {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    cancelled: AtomicBool,
    notify: Notify,
}

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Idempotent: cancelling twice is a no-op, so callers never have to track whether they already
    /// did (the read loop cancels on barge-in, on pause, and again on connection teardown).
    pub fn cancel(&self) {
        // Release here pairs with the Acquire in `is_cancelled`: everything the canceller wrote
        // before this point is visible to whoever observes the flag set.
        self.inner.cancelled.store(true, Ordering::Release);
        self.inner.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    /// Resolves as soon as `cancel()` has been called — immediately if it already has.
    ///
    /// `allow(dead_code)`: `main.rs`'s TTS send path used to race an enqueue against this future
    /// (`tokio::select!`) to unblock immediately when the outbound queue was full. It no longer
    /// can — that send now happens inside a synchronous, per-chunk `on_chunk` callback running on
    /// a blocking thread (see `spawn_speech`'s true-streaming rewrite), so it polls
    /// `is_cancelled()` between bounded retries instead. This async half of the type is kept
    /// (and still exercised below) as the general-purpose primitive for any future async waiter
    /// that needs to wake on a barge-in rather than poll for one.
    #[allow(dead_code)]
    pub async fn cancelled(&self) {
        loop {
            // Register the waiter BEFORE re-checking the flag. The reverse order loses a `cancel()`
            // that lands between the check and the registration, and this future would then hang
            // for the life of the connection.
            let notified = self.inner.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

impl CancelSignal for CancelFlag {
    fn is_cancelled(&self) -> bool {
        CancelFlag::is_cancelled(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn starts_uncancelled_and_latches_once_cancelled() {
        let flag = CancelFlag::new();
        assert!(!flag.is_cancelled());
        flag.cancel();
        assert!(flag.is_cancelled());
        flag.cancel(); // idempotent
        assert!(flag.is_cancelled());
    }

    #[test]
    fn a_clone_observes_the_original_cancel() {
        let flag = CancelFlag::new();
        let worker_copy = flag.clone();
        assert!(!worker_copy.is_cancelled());
        flag.cancel();
        assert!(worker_copy.is_cancelled());
    }

    #[test]
    fn blocking_code_sees_it_through_the_provider_trait() {
        // The HTTP provider only ever sees a `&dyn CancelSignal`; this is the path it takes.
        let flag = CancelFlag::new();
        let signal: &dyn CancelSignal = &flag;
        assert!(!signal.is_cancelled());
        flag.cancel();
        assert!(signal.is_cancelled());
    }

    #[tokio::test]
    async fn cancelled_resolves_immediately_when_already_cancelled() {
        let flag = CancelFlag::new();
        flag.cancel();
        // No timeout wrapper: if this ever hangs, the test harness itself reports the hang, which
        // is a louder failure than a timeout assertion that could pass on a slow machine.
        flag.cancelled().await;
    }

    #[tokio::test]
    async fn cancelled_wakes_a_waiter_registered_before_the_cancel() {
        let flag = CancelFlag::new();
        let waiter = flag.clone();
        let handle = tokio::spawn(async move { waiter.cancelled().await });
        // Give the spawned task a chance to park inside `notified()` before cancelling, so this
        // exercises the wake path rather than the already-cancelled shortcut above.
        tokio::time::sleep(Duration::from_millis(20)).await;
        flag.cancel();
        handle.await.expect("waiter task completes");
    }

    #[tokio::test]
    async fn cancelled_does_not_lose_a_cancel_racing_the_registration() {
        // The ordering bug this guards against: check-then-register loses a cancel that lands in
        // between. Run it enough times that a regression shows up as a hung test, not a flake.
        for _ in 0..200 {
            let flag = CancelFlag::new();
            let waiter = flag.clone();
            let handle = tokio::spawn(async move { waiter.cancelled().await });
            flag.cancel();
            handle.await.expect("waiter task completes");
        }
    }
}
