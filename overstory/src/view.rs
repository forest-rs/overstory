// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reusable authoring units for building retained UI subtrees.
//!
//! A [`View`] is not a runtime widget. It is an app-authoring component that
//! receives a scoped [`ChildUi`](crate::ChildUi) and appends ordinary controls.
//! The retained tree still contains elements and widgets created through
//! [`AppendSpec`](crate::AppendSpec).
//!
//! ```
//! use overstory::prelude::*;
//!
//! struct DeploymentView;
//!
//! struct DeploymentElements {
//!     card: ElementId,
//!     deploy: ElementId,
//! }
//!
//! impl View for DeploymentView {
//!     type Output = DeploymentElements;
//!
//!     fn build(self, ui: &mut ChildUi<'_>) -> Self::Output {
//!         let (card, deploy) = ui.children(Panel::column(16.0), |card| {
//!             let deploy_count: LocalState<u32> = card.local_state(0);
//!
//!             card.add(Text::new("Deployment").font_size(18.0));
//!
//!             card.with_child(Panel::row(12.0).align_center(), |row| {
//!                 row.add(ToggleSwitch::new().checked(true));
//!                 row.add(Text::new("Live rollout"));
//!             });
//!
//!             card.add_with_children(Panel::column(6.0), |status| {
//!                 status.add(Text::new("").bind_text(
//!                     deploy_count.map(|count| format!("Deploy attempts: {count}")),
//!                 ));
//!                 status.add(Slider::new().range(0.0, 100.0).value(42.0));
//!             });
//!
//!             let _ = card.content(Expander::new("Advanced").expanded(true), |advanced| {
//!                 advanced.add(CheckBox::new("Require review").checked(true));
//!                 advanced.add(CheckBox::new("Attach timing traces"));
//!             });
//!
//!             card.with_child(Panel::row(10.0).align_center(), |actions| {
//!                 actions.add(
//!                     Button::new("Deploy").on_activate(move |ui, _event| {
//!                         deploy_count.update(ui, |count| *count += 1);
//!                     }),
//!                 )
//!             })
//!         });
//!
//!         DeploymentElements { card, deploy }
//!     }
//! }
//!
//! let mut ui = Ui::new();
//! let root = ui.root();
//! let elements = ui.view(root, DeploymentView);
//!
//! assert_eq!(ui.get(root).unwrap().children(), &[elements.card]);
//! assert!(ui.get(elements.deploy).unwrap().parent().is_some());
//! ```

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
