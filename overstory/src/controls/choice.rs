// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared choice-option metadata for single-selection controls.
//!
//! Roadmap:
//! - Keep this module as the common label/enabled option model for
//!   `RadioGroup`, `Select`, `ListBox`, and segmented selection.
//! - Add typed option keys only when the broader item-control API is ready, so
//!   selection controls share one value model instead of each growing a local
//!   generic.
//! - Keep layout, virtualization, popup behavior, templates, and validation in
//!   the controls that own those concerns.

use alloc::string::String;
use alloc::vec::Vec;

/// Label and availability metadata for a selectable choice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceOption {
    label: String,
    enabled: bool,
}

impl ChoiceOption {
    /// Creates an enabled option with the given visible label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
        }
    }

    /// Returns the visible label for this option.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Consumes this option and returns its visible label.
    #[must_use]
    pub fn into_label(self) -> String {
        self.label
    }

    /// Returns whether this option can be selected by user input.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets whether this option can be selected by user input.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Convenience for [`Self::enabled`] with `false`.
    #[must_use]
    pub const fn disabled(self) -> Self {
        self.enabled(false)
    }
}

impl<S> From<S> for ChoiceOption
where
    S: Into<String>,
{
    fn from(value: S) -> Self {
        Self::new(value)
    }
}

pub(crate) fn collect_options<I, O>(options: I) -> Vec<ChoiceOption>
where
    I: IntoIterator<Item = O>,
    O: Into<ChoiceOption>,
{
    options.into_iter().map(Into::into).collect()
}

pub(crate) fn enabled_index(options: &[ChoiceOption], index: usize) -> Option<usize> {
    options
        .get(index)
        .is_some_and(ChoiceOption::is_enabled)
        .then_some(index)
}

pub(crate) fn selected_enabled_index(
    options: &[ChoiceOption],
    index: Option<usize>,
) -> Option<usize> {
    enabled_index(options, index?)
}

pub(crate) fn enabled_selection_target(
    options: &[ChoiceOption],
    index: Option<usize>,
) -> Option<Option<usize>> {
    match index {
        Some(index) => enabled_index(options, index).map(Some),
        None => Some(None),
    }
}

/// Returns the effective selected index for a requested target.
///
/// Invalid targets preserve the current selection so direct property writes,
/// bindings, and public setters all converge on one settled semantic value.
pub(crate) fn normalized_selection_target(
    options: &[ChoiceOption],
    requested: Option<usize>,
    current: Option<usize>,
) -> Option<usize> {
    enabled_selection_target(options, requested).unwrap_or(current)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedChoiceSelection {
    pub(crate) index: Option<usize>,
    pub(crate) label: Option<String>,
}

impl ResolvedChoiceSelection {
    pub(crate) fn from_enabled_target(
        options: &[ChoiceOption],
        index: Option<usize>,
    ) -> Option<Self> {
        let index = enabled_selection_target(options, index)?;
        Some(Self {
            index,
            label: option_label(options, index),
        })
    }

    pub(crate) fn changed_from(self, current: Option<usize>) -> Option<Self> {
        (self.index != current).then_some(self)
    }
}

pub(crate) fn option_label(options: &[ChoiceOption], index: Option<usize>) -> Option<String> {
    options.get(index?).map(|option| option.label.clone())
}

pub(crate) fn option_enabled(options: &[ChoiceOption], index: usize) -> bool {
    options.get(index).is_some_and(ChoiceOption::is_enabled)
}

pub(crate) fn first_enabled_index(options: &[ChoiceOption]) -> Option<usize> {
    options.iter().position(ChoiceOption::is_enabled)
}

pub(crate) fn last_enabled_index(options: &[ChoiceOption]) -> Option<usize> {
    options.iter().rposition(ChoiceOption::is_enabled)
}

pub(crate) fn next_enabled_index(
    options: &[ChoiceOption],
    current: Option<usize>,
) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    let start = current.map_or(0, |index| index.saturating_add(1));
    enabled_index_at_or_after(options, start)
        .or_else(|| current.and_then(|index| enabled_index(options, index)))
        .or_else(|| last_enabled_index(options))
}

pub(crate) fn previous_enabled_index(
    options: &[ChoiceOption],
    current: Option<usize>,
) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    let start = current
        .map(|index| index.saturating_sub(1))
        .unwrap_or(options.len().saturating_sub(1));
    enabled_index_at_or_before(options, start)
        .or_else(|| current.and_then(|index| enabled_index(options, index)))
        .or_else(|| first_enabled_index(options))
}

pub(crate) fn enabled_index_at_or_after(options: &[ChoiceOption], index: usize) -> Option<usize> {
    (index..options.len()).find(|candidate| option_enabled(options, *candidate))
}

pub(crate) fn enabled_index_at_or_before(options: &[ChoiceOption], index: usize) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    let index = index.min(options.len() - 1);
    (0..=index)
        .rev()
        .find(|candidate| option_enabled(options, *candidate))
}

pub(crate) fn wrapping_next_enabled_index(
    options: &[ChoiceOption],
    current: Option<usize>,
) -> Option<usize> {
    wrapping_enabled_index(options, current, 1)
}

pub(crate) fn wrapping_previous_enabled_index(
    options: &[ChoiceOption],
    current: Option<usize>,
) -> Option<usize> {
    wrapping_enabled_index(options, current, options.len().saturating_sub(1))
}

fn wrapping_enabled_index(
    options: &[ChoiceOption],
    current: Option<usize>,
    step: usize,
) -> Option<usize> {
    let len = options.len();
    if len == 0 {
        return None;
    }
    let start = current.map_or(0, |index| (index + step) % len);
    (0..len)
        .map(|offset| (start + offset) % len)
        .find(|index| option_enabled(options, *index))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[test]
    fn navigation_skips_disabled_options_without_wrapping() {
        let options = collect_options([
            ChoiceOption::new("A"),
            ChoiceOption::new("B").disabled(),
            ChoiceOption::new("C"),
        ]);

        assert_eq!(next_enabled_index(&options, Some(0)), Some(2));
        assert_eq!(next_enabled_index(&options, Some(2)), Some(2));
        assert_eq!(previous_enabled_index(&options, Some(2)), Some(0));
        assert_eq!(previous_enabled_index(&options, Some(0)), Some(0));
    }

    #[test]
    fn wrapping_navigation_skips_disabled_options() {
        let options = collect_options([
            ChoiceOption::new("A"),
            ChoiceOption::new("B").disabled(),
            ChoiceOption::new("C"),
        ]);

        assert_eq!(wrapping_next_enabled_index(&options, Some(2)), Some(0));
        assert_eq!(wrapping_previous_enabled_index(&options, Some(0)), Some(2));
    }

    #[test]
    fn resolved_choice_selection_filters_disabled_targets_and_detects_change() {
        let options = collect_options([
            ChoiceOption::new("A"),
            ChoiceOption::new("B").disabled(),
            ChoiceOption::new("C"),
        ]);

        assert_eq!(
            ResolvedChoiceSelection::from_enabled_target(&options, Some(2)),
            Some(ResolvedChoiceSelection {
                index: Some(2),
                label: Some(String::from("C")),
            })
        );
        assert_eq!(
            ResolvedChoiceSelection::from_enabled_target(&options, Some(1)),
            None
        );
        assert!(
            ResolvedChoiceSelection::from_enabled_target(&options, Some(2))
                .and_then(|selection| selection.changed_from(Some(2)))
                .is_none()
        );
    }

    #[test]
    fn normalized_selection_target_preserves_current_for_invalid_requests() {
        let options = collect_options([
            ChoiceOption::new("A"),
            ChoiceOption::new("B").disabled(),
            ChoiceOption::new("C"),
        ]);

        assert_eq!(
            normalized_selection_target(&options, Some(2), Some(0)),
            Some(2)
        );
        assert_eq!(
            normalized_selection_target(&options, Some(1), Some(0)),
            Some(0)
        );
        assert_eq!(
            normalized_selection_target(&options, Some(99), Some(0)),
            Some(0)
        );
        assert_eq!(normalized_selection_target(&options, None, Some(0)), None);
    }
}
