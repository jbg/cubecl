//! Resources submitted to a device may only be released after proven completion.

/// Owns resources while native work may still reference them. Cancellation,
/// unwinding, and a failed wait do not establish completion: those paths retain
/// the resources permanently. The successful fence path explicitly releases
/// them. Holding the owning service here also keeps its native pools alive.
pub(crate) struct CompletionRetention<T>(Option<T>);

impl<T> CompletionRetention<T> {
    pub(crate) fn new(resources: T) -> Self {
        Self(Some(resources))
    }

    pub(crate) fn release(mut self) -> T {
        self.0
            .take()
            .expect("completion resources should be retained")
    }

    #[cfg(multi_threading)]
    pub(crate) fn get_mut(&mut self) -> &mut T {
        self.0
            .as_mut()
            .expect("completion resources should be retained")
    }
}

impl<T> Drop for CompletionRetention<T> {
    fn drop(&mut self) {
        if let Some(resources) = self.0.take() {
            core::mem::forget(resources);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Resource(Arc<AtomicUsize>);
    impl Drop for Resource {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn successful_completion_releases_resources() {
        let drops = Arc::new(AtomicUsize::new(0));
        let held = CompletionRetention::new(Resource(drops.clone()));
        drop(held.release());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cancellation_and_unwind_do_not_release_in_flight_resources() {
        let drops = Arc::new(AtomicUsize::new(0));
        let held = CompletionRetention::new(Resource(drops.clone()));
        drop(held);
        let other = drops.clone();
        let _ = std::panic::catch_unwind(move || {
            let _held = CompletionRetention::new(Resource(other));
            panic!("native wait failed");
        });
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
}
