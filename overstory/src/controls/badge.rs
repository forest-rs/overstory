// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Compact non-interactive status content.
//!
//! A `Badge` owns its pill-like surface and one ordinary retained child. It
//! deliberately does not own placement over another control; use
//! [`crate::Badged`] for that composition.

use alloc::{boxed::Box, format, string::String};

use understory_style::TypeTag;

use crate::Text;
use crate::element::{Element, ElementId};
use crate::layouts::Layout;
use crate::ui::Ui;
use crate::widget::{AppendSpec, ElementConfig, ElementSpec, Widget};

/// Selector identity for [`Badge`].
pub const TYPE_BADGE: TypeTag = TypeTag(33);

/// Formatting policy for a numeric badge above a display limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadgeCountOverflow {
    /// Always display the exact count.
    Exact,
    /// Cap the displayed value at `max`.
    Cap {
        /// Largest number rendered directly.
        max: u32,
        /// Append `+` when the actual count exceeds `max`.
        show_plus: bool,
    },
}

impl Default for BadgeCountOverflow {
    fn default() -> Self {
        Self::Cap {
            max: 99,
            show_plus: true,
        }
    }
}

/// Builder for a themed, non-interactive badge containing one child.
pub struct Badge<Child> {
    child: Child,
    element: ElementConfig,
}

impl<Child: core::fmt::Debug> core::fmt::Debug for Badge<Child> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Badge")
            .field("child", &self.child)
            .finish_non_exhaustive()
    }
}

impl<Child> Badge<Child> {
    /// Creates a badge around an arbitrary retained child.
    #[must_use]
    pub fn new(child: Child) -> Self {
        let mut element = ElementConfig::default();
        element.set_layout(Layout::row(0.0).align_center().justify_center());
        Self { child, element }
    }
}

impl Badge<Text> {
    /// Creates a text badge.
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        Self::new(Text::new(text))
    }

    /// Creates a numeric badge with the default `99+` overflow policy.
    #[must_use]
    pub fn count(count: u32) -> Self {
        Self::count_with_overflow(count, BadgeCountOverflow::default())
    }

    /// Creates a numeric badge with an explicit overflow policy.
    #[must_use]
    pub fn count_with_overflow(count: u32, overflow: BadgeCountOverflow) -> Self {
        Self::with_text(format_count(count, overflow))
    }

    /// Creates a numeric badge only when `count` is non-zero.
    #[must_use]
    pub fn count_nonzero(count: u32) -> Option<Self> {
        (count != 0).then(|| Self::count(count))
    }
}

impl<Child> AppendSpec for Badge<Child>
where
    Child: AppendSpec,
{
    fn append_to(self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let id = ui.append_widget(parent, Box::new(BadgeWidget));
        self.element.apply_to(ui, id);
        ui.append(id, self.child);
        id
    }
}

impl<Child> ElementSpec for Badge<Child>
where
    Child: AppendSpec,
{
    fn element_config_mut(&mut self) -> &mut ElementConfig {
        &mut self.element
    }
}

#[derive(Debug)]
struct BadgeWidget;

impl Widget for BadgeWidget {
    fn type_tag(&self) -> Option<TypeTag> {
        Some(TYPE_BADGE)
    }

    fn accessibility_role(
        &self,
        _element: &Element,
        _ctx: &crate::AccessibilityContext<'_>,
    ) -> Option<accesskit::Role> {
        Some(accesskit::Role::GenericContainer)
    }

    fn is_hit_test_visible(&self, _element: &Element) -> bool {
        false
    }

    fn is_hit_test_subtree_visible(&self, _element: &Element) -> bool {
        false
    }
}

fn format_count(count: u32, overflow: BadgeCountOverflow) -> String {
    match overflow {
        BadgeCountOverflow::Exact => format!("{count}"),
        BadgeCountOverflow::Cap { max, show_plus } if count > max && show_plus => {
            format!("{max}+")
        }
        BadgeCountOverflow::Cap { max, .. } if count > max => format!("{max}"),
        BadgeCountOverflow::Cap { .. } => format!("{count}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Button, Size};

    #[test]
    fn numeric_badges_apply_overflow_policy() {
        assert_eq!(
            format_count(120, BadgeCountOverflow::default()),
            String::from("99+")
        );
        assert_eq!(
            format_count(
                120,
                BadgeCountOverflow::Cap {
                    max: 50,
                    show_plus: false,
                },
            ),
            String::from("50")
        );
        assert_eq!(
            format_count(120, BadgeCountOverflow::Exact),
            String::from("120")
        );
        assert!(Badge::count_nonzero(0).is_none());
    }

    #[test]
    fn badge_disables_hit_testing_for_its_entire_subtree() {
        let mut ui = Ui::new();
        let root = ui.root();
        ui.set_layout(root, Layout::row(0.0).logical_size(160.0, 80.0));
        let badge = ui.append(root, Badge::new(Button::new("New")));
        let button = ui.get(badge).unwrap().children()[0];
        ui.compute_layout(Size::new(160.0, 80.0));

        let point = ui.effective_bounds(button).center();
        assert_ne!(ui.hit_test(point), Some(button));
        assert_ne!(ui.hit_test(point), Some(badge));
    }
}
