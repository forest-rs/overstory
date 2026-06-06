// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared authoring extension traits for append specs.

mod element;
mod range;
mod state;
mod style;
mod surface;
mod template;
mod text;

pub use element::{ElementSpec, ElementSpecExt};
pub use range::RangeSpecExt;
pub use state::ControlStateSpecExt;
pub use style::SurfaceStyleExt;
pub use surface::SurfaceSpecExt;
pub use template::{ControlTemplateSpec, ControlTemplateSpecExt};
pub use text::TextSpecExt;
