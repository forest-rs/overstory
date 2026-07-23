// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_presentation::BaseDirection;

use crate::element::Element;
use crate::layouts::LayoutDirection;
use crate::resolved_properties::ResolvedPropertySource;

/// Inherited paragraph base-direction policy.
///
/// Paragraph direction affects text preparation, logical text alignment, and
/// accessibility semantics. It is independent of [`LayoutDirection`]: an LTR
/// identifier can establish its own paragraph direction inside RTL
/// application chrome without changing surrounding layout, controls,
/// scrolling, or overlays.
///
/// Explicit direction supplies authoritative intent to the text presentation
/// boundary. Backend support determines whether that intent can also override
/// content-derived bidi ordering. Overstory's current Parley 0.9 lowerer uses
/// explicit direction for alignment and semantics while Parley continues to
/// infer visual bidi levels from the content.
///
/// Paragraph direction does not isolate arbitrary embedded runs. Mixed-
/// direction rich content still needs run-level bidi isolation from a rich
/// text representation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ParagraphDirection {
    /// Uses the inherited application [`LayoutDirection`].
    #[default]
    MatchLayout,
    /// Lets the text backend infer paragraph direction from the content.
    Auto,
    /// Establishes a left-to-right paragraph.
    Ltr,
    /// Establishes a right-to-left paragraph.
    Rtl,
}

impl ParagraphDirection {
    pub(crate) const fn resolve(self, layout: LayoutDirection) -> BaseDirection {
        match self {
            Self::MatchLayout if layout.is_rtl() => BaseDirection::Rtl,
            Self::MatchLayout => BaseDirection::Ltr,
            Self::Auto => BaseDirection::Auto,
            Self::Ltr => BaseDirection::Ltr,
            Self::Rtl => BaseDirection::Rtl,
        }
    }

    pub(crate) const fn inspection_summary(self) -> &'static str {
        match self {
            Self::MatchLayout => "match-layout",
            Self::Auto => "auto",
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        }
    }
}

pub(crate) fn paragraph_base_direction(
    element: &Element,
    source: &impl ResolvedPropertySource,
) -> BaseDirection {
    let properties = source.properties();
    let policy = source.resolve_value(element, properties.paragraph_direction);
    let layout = source.resolve_value(element, properties.layout_direction);
    policy.resolve(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_resolves_independently_from_application_flow() {
        assert_eq!(
            ParagraphDirection::MatchLayout.resolve(LayoutDirection::Ltr),
            BaseDirection::Ltr
        );
        assert_eq!(
            ParagraphDirection::MatchLayout.resolve(LayoutDirection::Rtl),
            BaseDirection::Rtl
        );
        assert_eq!(
            ParagraphDirection::Auto.resolve(LayoutDirection::Rtl),
            BaseDirection::Auto
        );
        assert_eq!(
            ParagraphDirection::Ltr.resolve(LayoutDirection::Rtl),
            BaseDirection::Ltr
        );
        assert_eq!(
            ParagraphDirection::Rtl.resolve(LayoutDirection::Ltr),
            BaseDirection::Rtl
        );
    }
}
