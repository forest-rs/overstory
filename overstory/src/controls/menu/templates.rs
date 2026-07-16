// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{
    SLOT_MENU_BUTTON_INDICATOR, SLOT_MENU_BUTTON_LABEL, SLOT_MENU_ITEM_INDICATOR,
    SLOT_MENU_ITEM_LABEL,
};
use crate::template::{TemplateNode, TemplateSpec, TemplateSpecRecipe};

static MENU_BUTTON_TEMPLATE: TemplateSpecRecipe =
    TemplateSpecRecipe::new(menu_button_template_spec);
static MENU_ITEM_TEMPLATE: TemplateSpecRecipe = TemplateSpecRecipe::new(menu_item_template_spec);

pub(crate) fn default_menu_button_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&MENU_BUTTON_TEMPLATE)
}

pub(crate) fn default_menu_item_template() -> crate::ControlTemplate {
    crate::ControlTemplate::from_static(&MENU_ITEM_TEMPLATE)
}

fn menu_button_template_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::content_presenter(SLOT_MENU_BUTTON_LABEL),
        TemplateNode::text_slot(SLOT_MENU_BUTTON_INDICATOR),
    ])
}

fn menu_item_template_spec() -> TemplateSpec {
    TemplateSpec::from_nodes([
        TemplateNode::content_presenter(SLOT_MENU_ITEM_LABEL),
        TemplateNode::text_slot(SLOT_MENU_ITEM_INDICATOR),
    ])
}
