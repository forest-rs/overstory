// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! App-authoring API surface.
//!
//! Most applications should import [`crate::prelude`] and write views through
//! [`ChildUi`]. This module is for focused imports: it groups the same
//! authoring traits by what they configure so downstream crates can opt into a
//! calm, explicit surface instead of importing toolkit internals.
//!
//! The canonical path is:
//!
//! 1. implement [`View`] for reusable sections;
//! 2. build children through [`ChildUi`];
//! 3. configure controls with shared spec extension traits;
//! 4. use state and bindings as value sources when properties should update
//!    without rebuilding the view.
//!
//! Runtime behavior still belongs to widgets, routed events, styling, layout,
//! and presentation. Authoring traits only configure the specs that create
//! retained elements.

/// Binding sources and binding append-spec helpers.
pub mod binding {
    pub use crate::PropertyEndpoint;
    pub use crate::binding::{
        BindingSpecExt, BoundSpec, ExternalSource, ExternalSourceId, LocalState, LocalStateId,
        ValueBinding,
    };
}

/// Element identity and shared element configuration.
pub mod element {
    pub use crate::element_ref::{ElementRef, ElementRefSpecExt, ReferencedSpec};
    pub use crate::spec_ext::{ElementSpec, ElementSpecExt};
}

/// Numeric range property helpers.
pub mod range {
    pub use crate::spec_ext::RangeSpecExt;
}

/// Semantic control-state property helpers.
pub mod state {
    pub use crate::spec_ext::ControlStateSpecExt;
}

/// Style token and style-builder helpers.
pub mod style {
    pub use crate::spec_ext::SurfaceStyleExt;
    pub use crate::{
        ClassId, PartTag, PseudoClassId, ResourceKey, StyleClass, StyleTokenSet, StyleVocabulary,
        TypeTag,
    };
}

/// Surface, paint, and composition property helpers.
pub mod surface {
    pub use crate::spec_ext::SurfaceSpecExt;
}

/// Control-template replacement helpers.
pub mod template {
    pub use crate::spec_ext::{ControlTemplateSpec, ControlTemplateSpecExt};
}

/// Text and font property helpers.
pub mod text {
    pub use crate::spec_ext::TextSpecExt;
}

/// Retained tree construction primitives.
pub mod tree {
    pub use crate::ui::ChildUi;
    pub use crate::view::View;
    pub use crate::widget::{AppendSpec, ContentSlotSpec};
}

pub use binding::{
    BindingSpecExt, BoundSpec, ExternalSource, ExternalSourceId, LocalState, LocalStateId,
    PropertyEndpoint, ValueBinding,
};
pub use element::{ElementRef, ElementRefSpecExt, ElementSpec, ElementSpecExt, ReferencedSpec};
pub use range::RangeSpecExt;
pub use state::ControlStateSpecExt;
pub use style::SurfaceStyleExt;
pub use surface::SurfaceSpecExt;
pub use template::{ControlTemplateSpec, ControlTemplateSpecExt};
pub use text::TextSpecExt;
pub use tree::{AppendSpec, ChildUi, ContentSlotSpec, View};
