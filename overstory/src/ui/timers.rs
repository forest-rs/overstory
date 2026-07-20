// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Timer scheduling and dispatch for retained `Ui` elements.

use alloc::{boxed::Box, vec::Vec};

use understory_timing::{TimerDuration, TimerId, TimerInstant};

use crate::element::ElementId;
use crate::timer_runtime::TimerEvent;

use super::Ui;

impl Ui {
    /// Returns the host-supplied monotonic instant currently cached on the
    /// `Ui`.
    ///
    /// A host or direct embedding must advance this clock before untimestamped
    /// input or application mutation. Pointer dispatch also contributes its
    /// normalized event timestamp, and pumping timers advances it to the pump
    /// time. Templates and handlers can then schedule follow-on timers without
    /// threading `now` through every call.
    #[must_use]
    #[inline]
    pub fn now(&self) -> TimerInstant {
        self.timers.now()
    }

    /// Advances the cached monotonic time used by timers and motion.
    ///
    /// The clock never regresses: a value earlier than [`Self::now`] is
    /// ignored. This replaces the earlier rewindable behavior; deterministic
    /// callers that need a new origin should create a fresh `Ui` rather than
    /// rewinding a live one.
    ///
    /// Direct embeddings must call this before untimestamped external work. The
    /// companion `UiHost` does so at its dispatch and mutable-access
    /// boundaries. Pointer dispatch additionally advances from timestamps
    /// carried by normalized pointer state, and [`Self::pump_timers`] advances
    /// from its explicit argument.
    pub fn set_now(&mut self, now: TimerInstant) {
        let now = self.timers.set_now(now);
        self.advance_timeline(now);
    }

    /// Schedules a one-shot timer targeted at `target`. Returns the
    /// queue's id so callers can cancel it. The deadline is computed
    /// relative to [`Self::now`].
    pub fn schedule_timer_once(&mut self, target: ElementId, delay: TimerDuration) -> TimerId {
        self.timers.schedule_once(target, delay)
    }

    /// Schedules a repeating timer targeted at `target` that
    /// coalesces missed intervals (the typical UI-animation policy
    /// from [`understory_timing::TimerRepeat::coalescing`]). The first fire is `delay`
    /// from now; subsequent fires are `delay` apart relative to the
    /// previous drain time.
    pub fn schedule_timer_repeating(&mut self, target: ElementId, delay: TimerDuration) -> TimerId {
        self.timers.schedule_repeating(target, delay)
    }

    /// Cancels a pending timer. Returns `true` if a pending timer
    /// was removed. Idempotent.
    pub fn cancel_timer(&mut self, timer: TimerId) -> bool {
        self.timers.cancel(timer)
    }

    /// Returns the next pending timer deadline, if any. Hosts use
    /// this to arm their wakeup mechanism (e.g.
    /// `winit::ControlFlow::WaitUntil`).
    #[must_use]
    pub fn next_timer_deadline(&self) -> Option<TimerInstant> {
        self.timers.next_deadline()
    }

    /// Registers a timer handler on `id`. Fires whenever any timer
    /// scheduled against `id` reaches its deadline and is drained by
    /// [`Self::pump_timers`]. One handler per element; subsequent
    /// calls replace.
    pub fn on_timer<F>(&mut self, id: ElementId, handler: F)
    where
        F: FnMut(&mut Self, TimerEvent) + 'static,
    {
        self.timers.on_handler(id, Box::new(handler));
    }

    /// Removes a timer handler previously registered with
    /// [`Self::on_timer`]. No-op if none is registered.
    pub fn remove_timer_handler(&mut self, id: ElementId) {
        self.timers.remove_handler(id);
    }

    /// Drains every timer due at or before the effective UI time, dispatching
    /// each to its target's registered handler. Repeating timers re-arm only
    /// when the target is still alive and still has a timer handler after
    /// dispatch.
    ///
    /// This first advances the cached clock to at least `now`; an older
    /// argument cannot rewind dispatch time.
    pub fn pump_timers(&mut self, now: TimerInstant) {
        self.set_now(now);
        let now = self.now();
        let mut rearm = Vec::new();
        while let Some(timer) = self.timers.pop_expired(now) {
            let target = *timer.target();
            let id = timer.id();
            let repeating = timer.repeat().is_repeating();
            let mut should_rearm = false;
            // Take the handler out so it can borrow `&mut Ui`; put
            // back unless the handler removed itself or the element
            // vanished.
            if let Some(mut handler) = self.timers.take_handler(target) {
                let event = TimerEvent {
                    target,
                    timer: id,
                    now,
                };
                let previous_dispatching = self.timers.begin_dispatch(target);
                handler(self, event);
                self.timers.end_dispatch(previous_dispatching);

                let target_alive = self.get(target).is_some();
                let handler_removed = self.timers.take_handler_removed(target);
                if target_alive {
                    if !handler_removed && !self.timers.has_handler(target) {
                        self.timers.insert_handler_if_absent(target, handler);
                    }
                    should_rearm = repeating && self.timers.has_handler(target);
                }
            }
            if should_rearm {
                rearm.push(timer);
            }
        }
        for timer in rearm {
            self.timers.rearm(timer);
        }
    }
}
