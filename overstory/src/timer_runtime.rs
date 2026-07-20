// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained timer queue and per-element timer handlers.
//!
//! Roadmap:
//! - Keep clocks host-supplied and monotonic so core stays portable across
//!   native, browser, tests, and future frame-pacing integrations.
//! - Share queue semantics with `understory_timing`; Overstory should own
//!   element targeting, handler dispatch, and re-entrancy bookkeeping.
//! - Unify timers, animation ticks, cursor blink, tooltip delays, and reduced
//!   motion policy around one scheduler story.
//! - Add diagnostics for pending timers, late timers, coalesced repeats, and
//!   handlers that reschedule too aggressively.

use alloc::boxed::Box;

use hashbrown::{HashMap, HashSet};
use understory_timing::{
    ExpiredTimer, TimerDuration, TimerId, TimerInstant, TimerQueue, TimerRepeat,
};

use crate::element::ElementId;
use crate::ui::Ui;

/// Callback invoked when a timer targeted at an element is drained.
pub(crate) type TimerHandler = Box<dyn FnMut(&mut Ui, TimerEvent)>;

/// Timer queue, current clock, and per-element timer handlers.
pub(crate) struct TimerRuntime {
    /// Pending timers ordered by absolute monotonic deadline.
    queue: TimerQueue<ElementId>,
    /// One retained timer handler per element.
    ///
    /// A handler receives every timer targeted at that element. Individual
    /// timer ids remain available through [`TimerEvent::timer`] when a control
    /// needs to distinguish multiple outstanding timers.
    handlers: HashMap<ElementId, TimerHandler>,
    /// Elements whose handler was removed while that handler was dispatching.
    ///
    /// The dispatcher temporarily takes the active handler out of `handlers` so
    /// it can invoke the callback with `&mut Ui`. If the callback calls
    /// `remove_timer_handler`, this set prevents the dispatcher from putting
    /// the taken handler back afterward.
    handler_removals: HashSet<ElementId>,
    /// Element whose timer handler is currently running, if any.
    ///
    /// This lets nested timer pumping preserve the outer dispatch context.
    dispatching_handler: Option<ElementId>,
    /// Cached host-supplied monotonic instant used by relative scheduling APIs.
    now: TimerInstant,
}

impl TimerRuntime {
    /// Creates an empty timer runtime at monotonic instant zero.
    pub(crate) fn new() -> Self {
        Self {
            queue: TimerQueue::new(),
            handlers: HashMap::new(),
            handler_removals: HashSet::new(),
            dispatching_handler: None,
            now: 0,
        }
    }

    /// Returns the cached host monotonic instant.
    #[inline]
    pub(crate) fn now(&self) -> TimerInstant {
        self.now
    }

    /// Advances the cached host monotonic instant used for future schedules.
    #[inline]
    pub(crate) fn set_now(&mut self, now: TimerInstant) -> TimerInstant {
        self.now = self.now.max(now);
        self.now
    }

    /// Schedules a one-shot timer relative to the cached `now`.
    pub(crate) fn schedule_once(&mut self, target: ElementId, delay: TimerDuration) -> TimerId {
        self.queue.schedule_once(target, self.now, delay)
    }

    /// Schedules a coalescing repeating timer relative to the cached `now`.
    ///
    /// A zero delay is clamped to one tick because the underlying repeating
    /// policy requires a non-zero interval.
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

    /// Cancels a pending timer by id.
    pub(crate) fn cancel(&mut self, timer: TimerId) -> bool {
        self.queue.cancel(timer)
    }

    /// Returns the next pending timer deadline, if any.
    pub(crate) fn next_deadline(&self) -> Option<TimerInstant> {
        self.queue.next_deadline()
    }

    /// Registers or replaces the retained handler for `id`.
    pub(crate) fn on_handler(&mut self, id: ElementId, handler: TimerHandler) {
        self.handler_removals.remove(&id);
        self.handlers.insert(id, handler);
    }

    /// Removes the retained handler for `id`.
    ///
    /// If `id` is the handler currently being dispatched, remember the removal
    /// so the dispatcher does not reinsert the temporarily taken handler.
    pub(crate) fn remove_handler(&mut self, id: ElementId) {
        self.handlers.remove(&id);
        if self.dispatching_handler == Some(id) {
            self.handler_removals.insert(id);
        }
    }

    /// Removes all timer state owned by a removed element.
    ///
    /// Pending timers are discarded along with any retained or dispatch-time
    /// handler bookkeeping.
    pub(crate) fn remove_target(&mut self, id: ElementId) {
        self.handlers.remove(&id);
        self.handler_removals.remove(&id);
        self.queue.retain_pending(|timer| *timer.target() != id);
    }

    /// Pops the next timer due at or before `now`.
    pub(crate) fn pop_expired(&mut self, now: TimerInstant) -> Option<ExpiredTimer<ElementId>> {
        self.queue.pop_expired(now)
    }

    /// Rearms an expired repeating timer according to its repeat policy.
    pub(crate) fn rearm(&mut self, timer: ExpiredTimer<ElementId>) {
        let _ = self.queue.rearm(timer);
    }

    /// Takes the handler for dispatch so it may call back with `&mut Ui`.
    pub(crate) fn take_handler(&mut self, id: ElementId) -> Option<TimerHandler> {
        self.handlers.remove(&id)
    }

    /// Marks `id` as the handler currently being dispatched.
    ///
    /// Returns the previous dispatch context so callers can restore it after
    /// nested pumping or callback-driven re-entry.
    pub(crate) fn begin_dispatch(&mut self, id: ElementId) -> Option<ElementId> {
        self.dispatching_handler.replace(id)
    }

    /// Restores the dispatch context saved by [`Self::begin_dispatch`].
    pub(crate) fn end_dispatch(&mut self, previous: Option<ElementId>) {
        self.dispatching_handler = previous;
    }

    /// Returns and clears whether `id` removed itself during dispatch.
    pub(crate) fn take_handler_removed(&mut self, id: ElementId) -> bool {
        self.handler_removals.remove(&id)
    }

    /// Restores a taken handler only when no replacement was installed.
    pub(crate) fn insert_handler_if_absent(&mut self, id: ElementId, handler: TimerHandler) {
        self.handlers.entry(id).or_insert(handler);
    }

    /// Returns whether `id` currently has a retained handler.
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
