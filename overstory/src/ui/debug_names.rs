// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use understory_style::{ClassId, PseudoClassId, TypeTag};

use crate::inspection::{StyleTokenInspectionName, StyleTokenInspectionSnapshot};
use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_EXPANDED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED,
    PSEUDO_HOVER, PSEUDO_PLACEHOLDER_SHOWN, PSEUDO_PRESSED, PSEUDO_READ_ONLY, PSEUDO_SELECTED,
};
use crate::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, TYPE_ACCORDION, TYPE_ACCORDION_SECTION,
    TYPE_BUTTON, TYPE_CHECK_BOX, TYPE_DIVIDER, TYPE_EXPANDER, TYPE_GENERIC, TYPE_LIST_BOX,
    TYPE_LIST_ITEM, TYPE_OUTLINE_ITEM, TYPE_OUTLINE_VIEW, TYPE_POPUP, TYPE_PROGRESS_BAR,
    TYPE_RADIO_BUTTON, TYPE_RADIO_GROUP, TYPE_SCROLL_AREA, TYPE_SCROLL_BAR, TYPE_SCROLL_VIEW,
    TYPE_SEGMENTED_CONTROL, TYPE_SEGMENTED_ITEM, TYPE_SELECT, TYPE_SLIDER, TYPE_SPINNER,
    TYPE_SPLITTER, TYPE_TEXT, TYPE_TEXT_INPUT, TYPE_TOGGLE, TYPE_TOOLTIP, TYPE_VECTOR_PATH,
    TYPE_VIEWPORT, Ui,
};

#[derive(Debug, Default)]
pub(super) struct StyleTokenDebugNames {
    classes: Vec<StyleTokenInspectionName<ClassId>>,
    pseudos: Vec<StyleTokenInspectionName<PseudoClassId>>,
    type_tags: Vec<StyleTokenInspectionName<TypeTag>>,
}

impl StyleTokenDebugNames {
    fn snapshot(&self) -> StyleTokenInspectionSnapshot {
        StyleTokenInspectionSnapshot {
            classes: self.classes.clone(),
            pseudos: self.pseudos.clone(),
            type_tags: self.type_tags.clone(),
        }
    }
}

impl Ui {
    /// Assigns a human-readable debug name to a style class.
    ///
    /// Debug names are captured in inspection snapshots so devtools can show
    /// app-defined classes such as `.demo-card` instead of raw numeric ids.
    /// The name is diagnostics metadata only; it does not participate in
    /// selector matching or style resolution.
    pub fn set_class_debug_name(&mut self, class: ClassId, name: &'static str) {
        set_token_name(&mut self.style_debug_names.classes, class, name);
    }

    /// Assigns a human-readable debug name to a pseudoclass.
    ///
    /// Overstory registers names for its built-in pseudoclasses automatically.
    /// This method is for custom app or control states that should appear
    /// clearly in inspectors.
    pub fn set_pseudo_debug_name(&mut self, pseudo: PseudoClassId, name: &'static str) {
        set_token_name(&mut self.style_debug_names.pseudos, pseudo, name);
    }

    /// Assigns a human-readable debug name to a selector type tag.
    ///
    /// Built-in controls register their type-tag names automatically. Custom
    /// widgets can call this when they introduce their own selector identity.
    pub fn set_type_tag_debug_name(&mut self, type_tag: TypeTag, name: &'static str) {
        set_token_name(&mut self.style_debug_names.type_tags, type_tag, name);
    }

    pub(in crate::ui) fn style_token_inspection_snapshot(&self) -> StyleTokenInspectionSnapshot {
        self.style_debug_names.snapshot()
    }

    pub(in crate::ui) fn register_builtin_style_debug_names(&mut self) {
        self.set_pseudo_debug_name(PSEUDO_HOVER, ":hover");
        self.set_pseudo_debug_name(PSEUDO_PRESSED, ":pressed");
        self.set_pseudo_debug_name(PSEUDO_FOCUSED, ":focused");
        self.set_pseudo_debug_name(PSEUDO_DISABLED, ":disabled");
        self.set_pseudo_debug_name(PSEUDO_CHECKED, ":checked");
        self.set_pseudo_debug_name(PSEUDO_READ_ONLY, ":read-only");
        self.set_pseudo_debug_name(PSEUDO_FOCUS_VISIBLE, ":focus-visible");
        self.set_pseudo_debug_name(PSEUDO_SELECTED, ":selected");
        self.set_pseudo_debug_name(PSEUDO_PLACEHOLDER_SHOWN, ":placeholder-shown");
        self.set_pseudo_debug_name(PSEUDO_EXPANDED, ":expanded");

        self.set_class_debug_name(CLASS_SLIDER_MAJOR_TICK, ".slider-major-tick");
        self.set_class_debug_name(CLASS_SLIDER_MINOR_TICK, ".slider-minor-tick");

        self.set_type_tag_debug_name(TYPE_GENERIC, "Panel");
        self.set_type_tag_debug_name(TYPE_TEXT, "Text");
        self.set_type_tag_debug_name(TYPE_BUTTON, "Button");
        self.set_type_tag_debug_name(TYPE_TOGGLE, "ToggleSwitch");
        self.set_type_tag_debug_name(TYPE_DIVIDER, "Divider");
        self.set_type_tag_debug_name(TYPE_SPINNER, "Spinner");
        self.set_type_tag_debug_name(TYPE_SLIDER, "Slider");
        self.set_type_tag_debug_name(TYPE_PROGRESS_BAR, "ProgressBar");
        self.set_type_tag_debug_name(TYPE_CHECK_BOX, "CheckBox");
        self.set_type_tag_debug_name(TYPE_TEXT_INPUT, "TextInput");
        self.set_type_tag_debug_name(TYPE_SPLITTER, "Splitter");
        self.set_type_tag_debug_name(TYPE_LIST_BOX, "ListBox");
        self.set_type_tag_debug_name(TYPE_LIST_ITEM, "ListItem");
        self.set_type_tag_debug_name(TYPE_SCROLL_VIEW, "ScrollView");
        self.set_type_tag_debug_name(TYPE_SCROLL_BAR, "ScrollBar");
        self.set_type_tag_debug_name(TYPE_TOOLTIP, "Tooltip");
        self.set_type_tag_debug_name(TYPE_POPUP, "Popup");
        self.set_type_tag_debug_name(TYPE_VIEWPORT, "Viewport");
        self.set_type_tag_debug_name(TYPE_SELECT, "Select");
        self.set_type_tag_debug_name(TYPE_VECTOR_PATH, "VectorPath");
        self.set_type_tag_debug_name(TYPE_SCROLL_AREA, "ScrollArea");
        self.set_type_tag_debug_name(TYPE_OUTLINE_VIEW, "OutlineView");
        self.set_type_tag_debug_name(TYPE_OUTLINE_ITEM, "OutlineItem");
        self.set_type_tag_debug_name(TYPE_RADIO_BUTTON, "RadioButton");
        self.set_type_tag_debug_name(TYPE_RADIO_GROUP, "RadioGroup");
        self.set_type_tag_debug_name(TYPE_EXPANDER, "Expander");
        self.set_type_tag_debug_name(TYPE_SEGMENTED_CONTROL, "SegmentedControl");
        self.set_type_tag_debug_name(TYPE_SEGMENTED_ITEM, "SegmentedItem");
        self.set_type_tag_debug_name(TYPE_ACCORDION, "Accordion");
        self.set_type_tag_debug_name(TYPE_ACCORDION_SECTION, "AccordionSection");
    }
}

fn set_token_name<T>(names: &mut Vec<StyleTokenInspectionName<T>>, id: T, name: &'static str)
where
    T: Copy + Eq,
{
    if let Some(existing) = names.iter_mut().find(|existing| existing.id == id) {
        existing.name = name;
    } else {
        names.push(StyleTokenInspectionName { id, name });
    }
}
