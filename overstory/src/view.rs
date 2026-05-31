// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reusable authoring units for building retained UI subtrees.
//!
//! A [`View`] is not a runtime widget. It is an app-authoring component that
//! receives a scoped [`ChildUi`](crate::ChildUi) and appends ordinary controls.
//! The retained tree still contains elements and widgets created through
//! [`AppendSpec`](crate::AppendSpec).

use crate::ui::ChildUi;

/// Authoring component that builds a subtree under a scoped parent.
///
/// Implement `View` for reusable app sections, pages, or custom composition
/// helpers. A view can return any value it needs, such as an
/// [`ElementRef`](crate::ElementRef) or a small handle object, while still
/// building through the normal retained-tree append APIs.
pub trait View {
    /// Value returned after the view builds.
    type Output;

    /// Appends this view's controls into `ui`.
    fn build(self, ui: &mut ChildUi<'_>) -> Self::Output;
}
