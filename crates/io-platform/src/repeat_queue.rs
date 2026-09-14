//! A repeat is disposable: one pending step per source, fresh input required at execution.
use io_core::exclusive_depth::{RepeatSource, RepeatWindow};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct RepeatQueue {
    windows: Vec<(RepeatSource, Instant)>,
    pending: BTreeSet<(u64, RepeatSource)>,
}
impl RepeatQueue {
    pub fn refresh(&mut self, windows: Vec<RepeatWindow>, now: Instant) {
        self.windows = windows
            .into_iter()
            .map(|w| (w.source, now + Duration::from_millis(w.remaining_ms)))
            .collect();
    }
    fn active(&self, source: RepeatSource, now: Instant) -> bool {
        self.windows
            .iter()
            .any(|(s, until)| *s == source && now < *until)
    }
    pub fn reserve(&mut self, epoch: u64, source: RepeatSource, now: Instant) -> bool {
        self.active(source, now) && self.pending.insert((epoch, source))
    }
    pub fn discard(&mut self, epoch: u64, source: RepeatSource) {
        self.pending.remove(&(epoch, source));
    }
    pub fn take(
        &mut self,
        epoch: u64,
        source: RepeatSource,
        queued_at: Instant,
        now: Instant,
    ) -> bool {
        self.discard(epoch, source);
        now.saturating_duration_since(queued_at) < Duration::from_millis(50)
            && self.active(source, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeats_coalesce_and_release_or_a_new_hold_invalidates_queued_steps() {
        let mut q = RepeatQueue::default();
        let now = Instant::now();
        let source = RepeatSource {
            slot: 105,
            cycle: 1,
        };
        q.refresh(
            vec![RepeatWindow {
                source,
                remaining_ms: 100,
            }],
            now,
        );
        assert!(q.reserve(0, source, now));
        assert!(!q.reserve(0, source, now));
        q.refresh(vec![], now);
        assert!(!q.take(0, source, now, now));
        let next = RepeatSource { cycle: 2, ..source };
        q.refresh(
            vec![RepeatWindow {
                source: next,
                remaining_ms: 100,
            }],
            now,
        );
        assert!(!q.take(0, source, now, now));
        assert!(q.reserve(0, next, now));
        assert!(q.take(0, next, now, now));
    }
    #[test]
    fn expired_input_and_backlogged_repeats_are_dropped_without_bursts() {
        let mut q = RepeatQueue::default();
        let now = Instant::now();
        let source = RepeatSource {
            slot: 108,
            cycle: 1,
        };
        q.refresh(
            vec![RepeatWindow {
                source,
                remaining_ms: 100,
            }],
            now,
        );
        assert!(q.reserve(0, source, now));
        assert!(!q.take(0, source, now, now + Duration::from_millis(50)));
        assert!(!q.reserve(0, source, now + Duration::from_millis(100)));
        q.refresh(
            vec![RepeatWindow {
                source,
                remaining_ms: 5,
            }],
            now,
        );
        assert!(!q.take(0, source, now, now + Duration::from_millis(5)));
        // Completing an old runtime epoch must not remove the new epoch's reservation.
        q.refresh(
            vec![RepeatWindow {
                source,
                remaining_ms: 100,
            }],
            now,
        );
        assert!(q.reserve(1, source, now));
        q.discard(0, source);
        assert!(!q.reserve(1, source, now));
    }
}
