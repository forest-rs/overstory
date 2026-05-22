// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;

use hashbrown::{HashMap, HashSet};
use understory_timing::{
    ExpiredTimer, TimerDuration, TimerId, TimerInstant, TimerQueue, TimerRepeat,
};

use crate::element::ElementId;
use crate::ui::Ui;

pub(crate) type TimerHandler = Box<dyn FnMut(&mut Ui, TimerEvent)>;

/// Timer queue, current clock, and per-element timer handlers.
pub(crate) struct TimerRuntime {
    queue: TimerQueue<ElementId>,
    handlers: HashMap<ElementId, TimerHandler>,
    handler_removals: HashSet<ElementId>,
    dispatching_handler: Option<ElementId>,
    now: TimerInstant,
}

impl TimerRuntime {
    pub(crate) fn new() -> Self {
        Self {
            queue: TimerQueue::new(),
            handlers: HashMap::new(),
            handler_removals: HashSet::new(),
            dispatching_handler: None,
            now: 0,
        }
    }

    #[inline]
    pub(crate) fn now(&self) -> TimerInstant {
        self.now
    }

    #[inline]
    pub(crate) fn set_now(&mut self, now: TimerInstant) {
        self.now = now;
    }

    pub(crate) fn schedule_once(&mut self, target: ElementId, delay: TimerDuration) -> TimerId {
        self.queue.schedule_once(target, self.now, delay)
    }

    pub(crate) fn schedule_repeating(
        &mut self,
        target: ElementId,
        delay: TimerDuration,
    ) -> TimerId {
        let interval =
            core::num::NonZeroU64::new(delay.max(1)).expect("delay clamped to a non-zero value");
        self.queue
            .schedule(target, self.now, delay, TimerRepeat::coalescing(interval))
    }

    pub(crate) fn cancel(&mut self, timer: TimerId) -> bool {
        self.queue.cancel(timer)
    }

    pub(crate) fn next_deadline(&self) -> Option<TimerInstant> {
        self.queue.next_deadline()
    }

    pub(crate) fn on_handler(&mut self, id: ElementId, handler: TimerHandler) {
        self.handler_removals.remove(&id);
        self.handlers.insert(id, handler);
    }

    pub(crate) fn remove_handler(&mut self, id: ElementId) {
        self.handlers.remove(&id);
        if self.dispatching_handler == Some(id) {
            self.handler_removals.insert(id);
        }
    }

    pub(crate) fn remove_target(&mut self, id: ElementId) {
        self.handlers.remove(&id);
        self.handler_removals.remove(&id);
        self.queue.retain_pending(|timer| *timer.target() != id);
    }

    pub(crate) fn pop_expired(&mut self, now: TimerInstant) -> Option<ExpiredTimer<ElementId>> {
        self.queue.pop_expired(now)
    }

    pub(crate) fn rearm(&mut self, timer: ExpiredTimer<ElementId>) {
        let _ = self.queue.rearm(timer);
    }

    pub(crate) fn take_handler(&mut self, id: ElementId) -> Option<TimerHandler> {
        self.handlers.remove(&id)
    }

    pub(crate) fn begin_dispatch(&mut self, id: ElementId) -> Option<ElementId> {
        self.dispatching_handler.replace(id)
    }

    pub(crate) fn end_dispatch(&mut self, previous: Option<ElementId>) {
        self.dispatching_handler = previous;
    }

    pub(crate) fn take_handler_removed(&mut self, id: ElementId) -> bool {
        self.handler_removals.remove(&id)
    }

    pub(crate) fn insert_handler_if_absent(&mut self, id: ElementId, handler: TimerHandler) {
        self.handlers.entry(id).or_insert(handler);
    }

    pub(crate) fn has_handler(&self, id: ElementId) -> bool {
        self.handlers.contains_key(&id)
    }
}

/// Payload delivered to a timer handler when a scheduled timer fires.
///
/// `target` is the element the timer was scheduled against and the element
/// the handler is registered on; `timer` is the queue id for disambiguating
/// multiple timers on the same element; `now` is the monotonic instant at
/// which dispatch is happening.
#[allow(
    unnameable_types,
    reason = "re-exported from the crate root via lib.rs"
)]
#[derive(Copy, Clone, Debug)]
pub struct TimerEvent {
    /// The element the timer fired on.
    pub target: ElementId,
    /// The id assigned by [`TimerQueue`] when the timer was scheduled.
    ///
    /// Compare against a stored token to recognize stale deliveries.
    pub timer: TimerId,
    /// Host-supplied monotonic time of dispatch.
    pub now: TimerInstant,
}
