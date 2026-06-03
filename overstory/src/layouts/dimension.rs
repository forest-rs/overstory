// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Two-dimensional layout dimension.
///
/// `None` on a containing longhand means "no opinion". `Auto` means an
/// explicit auto-sized opinion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutDimension {
    /// Let the layout engine choose the dimension.
    Auto,
    /// Fixed logical-pixel dimension.
    Logical(f32),
    /// Fraction of the containing block, where `1.0` means 100%.
    Percent(f32),
}

impl LayoutDimension {
    pub(crate) fn inspection_summary(self) -> String {
        match self {
            Self::Auto => String::from("auto"),
            Self::Logical(value) => format!("{}px", scalar_summary(value)),
            Self::Percent(value) => format!("{}%", scalar_summary(value * 100.0)),
        }
    }

    pub(super) fn to_taffy(self) -> taffy::Dimension {
        match self {
            Self::Auto => taffy::Dimension::auto(),
            Self::Logical(value) => taffy::Dimension::length(value),
            Self::Percent(value) => taffy::Dimension::percent(value),
        }
    }
}

pub(crate) fn scalar_summary(value: f32) -> String {
    if value.fract().abs() < f32::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.3}")
    }
}

pub(crate) fn push_labeled_summary_part(
    parts: &mut Vec<String>,
    label: &str,
    value: impl Into<String>,
) {
    parts.push(format!("{label} {}", value.into()));
}

pub(crate) fn join_summary_parts(parts: Vec<String>, separator: &str) -> String {
    let mut iter = parts.into_iter();
    let Some(mut output) = iter.next() else {
        return String::from("none");
    };
    for part in iter {
        output.push_str(separator);
        output.push_str(&part);
    }
    output
}
