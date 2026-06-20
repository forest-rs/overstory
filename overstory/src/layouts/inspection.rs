// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub(crate) fn scalar_summary(value: impl Into<f64>) -> String {
    let value = value.into();
    if value.fract().abs() < f64::EPSILON {
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
