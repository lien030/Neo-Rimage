use crate::domain::{CancellationProbe, EngineProgressEvent, ProgressReporter};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Runtime services supplied by JobManager for one engine invocation.
pub struct EngineContext<'a> {
    pub progress: &'a dyn ProgressReporter,
    pub cancellation: &'a dyn CancellationProbe,
}

impl<'a> EngineContext<'a> {
    pub fn new(
        progress: &'a dyn ProgressReporter,
        cancellation: &'a dyn CancellationProbe,
    ) -> Self {
        Self {
            progress,
            cancellation,
        }
    }
}

#[derive(Clone, Default)]
pub struct AtomicCancellationToken {
    requested: Arc<AtomicBool>,
}

impl AtomicCancellationToken {
    pub fn request_cancel(&self) {
        self.requested.store(true, Ordering::Release);
    }

    pub fn reset(&self) {
        self.requested.store(false, Ordering::Release);
    }
}

impl CancellationProbe for AtomicCancellationToken {
    fn is_cancel_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancelled;

impl CancellationProbe for NeverCancelled {
    fn is_cancel_requested(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopProgressReporter;

impl ProgressReporter for NoopProgressReporter {
    fn report(&self, _event: EngineProgressEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_token_is_shared_and_cooperative() {
        let token = AtomicCancellationToken::default();
        let worker_view = token.clone();
        assert!(!worker_view.is_cancel_requested());

        token.request_cancel();
        assert!(worker_view.is_cancel_requested());

        token.reset();
        assert!(!worker_view.is_cancel_requested());
    }
}
