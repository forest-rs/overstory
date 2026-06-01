// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained property-binding graph for one UI tree.
//!
//! Roadmap:
//! - Keep bindings type-safe and property-oriented; templates and controls
//!   should not need an expression language to synchronize simple state.
//! - Make template bindings, app bindings, and future two-way bindings share
//!   one graph and one diagnostic story.
//! - Add cycle, stale endpoint, and binding-cost diagnostics before bindings
//!   become hard to debug in larger examples.
//! - Keep binding evaluation inside Overstory while leaving the dependency
//!   property substrate in `understory_property_binding`.

use alloc::vec::Vec;

use hashbrown::HashMap;
use understory_property::LocalValueSource;
use understory_property_binding::{
    BindingError, BindingId, BindingSet, EndpointKey, PropertyEndpoint,
};

use crate::binding::BindingOwner;
use crate::channels::BINDING;

/// One-way property bindings and their internal invalidation tracker.
pub(crate) struct BindingRuntime {
    /// Binding graph for retained elements.
    ///
    /// Temporarily `None` while `Ui::flush_bindings` owns the set and evaluates
    /// updates against `&mut Ui`.
    bindings: Option<BindingSet<BindingOwner>>,
    target_write_sources: HashMap<EndpointKey<BindingOwner>, LocalValueSource>,
    binding_targets: Vec<Option<BindingTargetRecord>>,
}

#[derive(Copy, Clone, Debug)]
struct BindingTargetRecord {
    source: EndpointKey<BindingOwner>,
    target: EndpointKey<BindingOwner>,
}

impl BindingRuntime {
    pub(crate) fn new() -> Self {
        Self {
            bindings: Some(BindingSet::new(BINDING)),
            target_write_sources: HashMap::new(),
            binding_targets: Vec::new(),
        }
    }

    pub(crate) fn bind<T>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, T>,
        target: PropertyEndpoint<BindingOwner, T>,
    ) -> Result<BindingId, BindingError<BindingOwner>>
    where
        T: Clone + 'static,
    {
        self.bind_map_with_target_source(
            source,
            target,
            T::clone,
            LocalValueSource::TemplateBinding,
        )
    }

    pub(crate) fn bind_map<S, T, F>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, S>,
        target: PropertyEndpoint<BindingOwner, T>,
        map: F,
    ) -> Result<BindingId, BindingError<BindingOwner>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
    {
        self.bind_map_with_target_source(source, target, map, LocalValueSource::TemplateBinding)
    }

    pub(crate) fn bind_map_with_target_source<S, T, F>(
        &mut self,
        source: PropertyEndpoint<BindingOwner, S>,
        target: PropertyEndpoint<BindingOwner, T>,
        map: F,
        write_source: LocalValueSource,
    ) -> Result<BindingId, BindingError<BindingOwner>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
    {
        let source_key = source.key();
        let target_key = target.key();
        let bindings = self
            .bindings
            .as_mut()
            .expect("bindings always Some outside flush_bindings");
        let id = bindings.bind_map(source, target, map)?;
        bindings.mark_binding_dirty(id);
        self.record_binding_target(id, source_key, target_key, write_source);
        Ok(id)
    }

    pub(crate) fn write_source_for_target(
        &self,
        target: EndpointKey<BindingOwner>,
    ) -> LocalValueSource {
        self.target_write_sources
            .get(&target)
            .copied()
            .unwrap_or(LocalValueSource::TemplateBinding)
    }

    /// Marks bindings that depend on `endpoint` dirty.
    ///
    /// No-ops while bindings are already being flushed; the active flush owns
    /// the binding set and records changes directly.
    pub(crate) fn mark_endpoint_changed(&mut self, endpoint: EndpointKey<BindingOwner>) -> bool {
        self.bindings
            .as_mut()
            .is_some_and(|bindings| bindings.mark_endpoint_changed(endpoint))
    }

    pub(crate) fn clear_owner(&mut self, owner: BindingOwner) {
        if let Some(bindings) = self.bindings.as_mut() {
            bindings.clear_owner(owner);
        }
        for record in &mut self.binding_targets {
            let Some(binding) = *record else {
                continue;
            };
            if binding.source.owner() == owner || binding.target.owner() == owner {
                self.target_write_sources.remove(&binding.target);
                *record = None;
            }
        }
    }

    /// Takes the binding graph for a flush pass.
    pub(crate) fn take(&mut self) -> BindingSet<BindingOwner> {
        self.bindings
            .take()
            .expect("bindings always Some outside flush_bindings")
    }

    /// Restores the binding graph after a flush pass.
    pub(crate) fn restore(&mut self, bindings: BindingSet<BindingOwner>) {
        self.bindings = Some(bindings);
    }

    fn record_binding_target(
        &mut self,
        id: BindingId,
        source: EndpointKey<BindingOwner>,
        target: EndpointKey<BindingOwner>,
        write_source: LocalValueSource,
    ) {
        let Ok(index) = usize::try_from(id.get()) else {
            return;
        };
        if self.binding_targets.len() <= index {
            self.binding_targets.resize_with(index + 1, || None);
        }
        self.binding_targets[index] = Some(BindingTargetRecord { source, target });
        self.target_write_sources.insert(target, write_source);
    }
}
