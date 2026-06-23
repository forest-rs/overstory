// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native UIKit Overstory host built on `UIViewController`, `CAMetalLayer`, and
//! `overstory_presenter_wgpu`.
//!
//! This crate owns the UIKit view/controller shell, UIKit input translation,
//! native display-link ticks, and native `wgpu` surface creation. Shared
//! `wgpu` / Vello presentation is delegated to `overstory_presenter_wgpu`.
//! It explicitly does not own Overstory UI semantics, app-bundle construction,
//! signing, or platform-independent frame scheduling policy.

extern crate alloc;

#[cfg(any(target_os = "ios", target_os = "tvos"))]
mod uikit;

#[cfg(any(target_os = "ios", target_os = "tvos"))]
pub use uikit::{HostResult, ViewOptions, create_root_view_controller};

#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
mod unsupported;

#[cfg(not(any(target_os = "ios", target_os = "tvos")))]
pub use unsupported::{HostResult, UnsupportedPlatform, ViewOptions, create_root_view_controller};
