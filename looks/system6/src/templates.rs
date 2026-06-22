// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Look-authored control templates.
//!
//! Proof-of-concept that a look can own a control's *structure*, not just its
//! styling, by setting the `control_template` property from its cascade (the
//! same mechanism the framework uses for its defaults). The look's
//! [`cascade`](crate::cascade) installs [`check_box`] on `TYPE_CHECK_BOX`,
//! overriding the framework's default template.
//!
//! The structure here matches the framework's checkbox contract (a box holding
//! the mark, beside the label); the value of owning the template is that a look
//! could diverge structurally (extra frames, different arrangements) without
//! the control's behavior changing — the control finds its parts by slot.

use overstory::{
    ControlTemplate, SLOT_BUTTON_CONTENT, SLOT_CHECK_BOX_BOX, SLOT_CHECK_BOX_LABEL,
    SLOT_CHECK_BOX_MARK, TemplateNode, TemplateSpec, TemplateSpecRecipe,
};

use crate::tokens::{CLASS_DEFAULT_BUTTON_INNER, CLASS_DEFAULT_BUTTON_OUTER};

static CHECK_BOX: TemplateSpecRecipe = TemplateSpecRecipe::new(check_box_spec);
static DEFAULT_BUTTON: TemplateSpecRecipe = TemplateSpecRecipe::new(default_button_spec);

/// The System 6 default-button template: an outer ring around an inner face,
/// giving the classic Mac double border. Styling alone can't draw two
/// concentric outlines with a gap, so this is a genuine template swap.
#[must_use]
pub fn default_button() -> ControlTemplate {
    ControlTemplate::from_static(&DEFAULT_BUTTON)
}

fn default_button_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([TemplateNode::surface()
        .with_class(CLASS_DEFAULT_BUTTON_OUTER)
        .with_child(
            TemplateNode::surface()
                .with_class(CLASS_DEFAULT_BUTTON_INNER)
                .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT)),
        )])
}

/// The System 6 checkbox template: a square box holding the mark, beside the
/// label. Returned as a stable-identity [`ControlTemplate`] for cascade use.
#[must_use]
pub fn check_box() -> ControlTemplate {
    ControlTemplate::from_static(&CHECK_BOX)
}

fn check_box_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::surface_slot(SLOT_CHECK_BOX_BOX)
            .with_child(TemplateNode::text_slot(SLOT_CHECK_BOX_MARK)),
        TemplateNode::content_presenter(SLOT_CHECK_BOX_LABEL),
    ])
}
