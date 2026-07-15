// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;

use crate::element::ElementId;
use crate::semantic_callback_runtime::{SEMANTIC_CALLBACK_BUDGET, SemanticCallback};

use super::Ui;

impl Ui {
    pub(crate) fn with_semantic_callback_boundary<R>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.semantic_callbacks.begin_boundary();
        let result = operation(self);
        self.semantic_callbacks.end_boundary();
        self.drain_semantic_callbacks();
        result
    }

    pub(crate) fn enqueue_semantic_callback(
        &mut self,
        source: ElementId,
        callback: impl FnOnce(&mut Self) + 'static,
    ) {
        self.semantic_callbacks
            .enqueue(source, Box::new(callback) as SemanticCallback);
        self.drain_semantic_callbacks();
    }

    fn drain_semantic_callbacks(&mut self) {
        if !self.semantic_callbacks.is_ready_to_drain() {
            return;
        }

        self.semantic_callbacks.set_draining(true);
        let mut dispatched = 0;
        loop {
            let Some(pending) = self.semantic_callbacks.pop_front() else {
                break;
            };
            if self
                .get(pending.source)
                .and_then(|element| element.widget())
                .is_none()
            {
                continue;
            }
            if dispatched == SEMANTIC_CALLBACK_BUDGET {
                let pending = self.semantic_callbacks.pending_len() + 1;
                self.semantic_callbacks.clear();
                self.semantic_callbacks.set_draining(false);
                panic!(
                    "semantic callback dispatch did not converge after \
                     {SEMANTIC_CALLBACK_BUDGET} callbacks ({pending} still pending); \
                     check for callbacks that repeatedly mutate each other's controls"
                );
            }
            dispatched += 1;
            (pending.callback)(self);
        }
        self.semantic_callbacks.set_draining(false);
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use core::cell::Cell;

    use crate::semantic_callback_runtime::SEMANTIC_CALLBACK_BUDGET;

    use super::Ui;

    #[test]
    fn a_finite_chain_may_use_the_entire_semantic_callback_budget() {
        let mut ui = Ui::new();
        let root = ui.root();
        let dispatched = Rc::new(Cell::new(0));

        ui.with_semantic_callback_boundary(|ui| {
            for _ in 0..SEMANTIC_CALLBACK_BUDGET {
                let dispatched = Rc::clone(&dispatched);
                ui.enqueue_semantic_callback(root, move |_ui| {
                    dispatched.set(dispatched.get() + 1);
                });
            }
        });

        assert_eq!(dispatched.get(), SEMANTIC_CALLBACK_BUDGET);
    }
}
