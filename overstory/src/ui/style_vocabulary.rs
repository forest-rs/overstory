// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;

use crate::pseudos::{
    PSEUDO_CHECKED, PSEUDO_DISABLED, PSEUDO_EXPANDED, PSEUDO_FOCUS_VISIBLE, PSEUDO_FOCUSED,
    PSEUDO_HOVER, PSEUDO_PLACEHOLDER_SHOWN, PSEUDO_PRESSED, PSEUDO_READ_ONLY, PSEUDO_SELECTED,
};
use crate::theme;
use crate::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_ACCORDION_SECTION_CONTENT,
    PART_ACCORDION_SECTION_DISCLOSURE, PART_ACCORDION_SECTION_HEADER, PART_ACCORDION_SECTION_LABEL,
    PART_BUTTON_LABEL, PART_CHECK_BOX_BOX, PART_CHECK_BOX_LABEL, PART_CHECK_BOX_MARK,
    PART_EXPANDER_CONTENT, PART_EXPANDER_DISCLOSURE, PART_EXPANDER_HEADER, PART_EXPANDER_LABEL,
    PART_LIST_ITEM_CONTENT, PART_LIST_ITEM_LABEL, PART_MENU_BUTTON_INDICATOR,
    PART_MENU_BUTTON_LABEL, PART_MENU_ITEM_INDICATOR, PART_MENU_ITEM_LABEL,
    PART_OUTLINE_ITEM_CONTENT, PART_OUTLINE_ITEM_DISCLOSURE, PART_OUTLINE_ITEM_LABEL,
    PART_PROGRESS_BAR_FILL, PART_PROGRESS_BAR_LABEL, PART_RADIO_BUTTON_DOT,
    PART_RADIO_BUTTON_INDICATOR, PART_RADIO_BUTTON_LABEL, PART_SCROLL_BAR_THUMB,
    PART_SEGMENTED_ITEM_LABEL, PART_SEGMENTED_SELECTION_INDICATOR, PART_SELECT_INDICATOR,
    PART_SELECT_LABEL, PART_SLIDER_AXIS, PART_SLIDER_THUMB, PART_SLIDER_TICK,
    PART_SLIDER_TICK_LABEL, PART_SLIDER_TRACK, PART_TEXT_INPUT_CARET, PART_TEXT_INPUT_SELECTION,
    PART_TEXT_INPUT_TEXT, PART_TOGGLE_THUMB, PART_TOOLTIP_LABEL, StyleClass, TYPE_ACCORDION,
    TYPE_ACCORDION_SECTION, TYPE_BUTTON, TYPE_CHECK_BOX, TYPE_DIVIDER, TYPE_EXPANDER, TYPE_GENERIC,
    TYPE_LIST_BOX, TYPE_LIST_ITEM, TYPE_MENU, TYPE_MENU_BUTTON, TYPE_MENU_ITEM, TYPE_OUTLINE_ITEM,
    TYPE_OUTLINE_VIEW, TYPE_POPUP, TYPE_PROGRESS_BAR, TYPE_RADIO_BUTTON, TYPE_RADIO_GROUP,
    TYPE_ROOT, TYPE_SCROLL_AREA, TYPE_SCROLL_BAR, TYPE_SCROLL_VIEW, TYPE_SEGMENTED_CONTROL,
    TYPE_SEGMENTED_ITEM, TYPE_SELECT, TYPE_SLIDER, TYPE_SPINNER, TYPE_SPLITTER, TYPE_TEXT,
    TYPE_TEXT_INPUT, TYPE_TOGGLE, TYPE_TOOLTIP, TYPE_VECTOR_PATH, TYPE_VIEWPORT, Ui,
};
use understory_style::{
    ClassId, PartTag, PseudoClassId, ResourceKey, StyleTokenSet, StyleVocabulary, TypeTag,
};

pub(crate) struct BuiltInStyleTokens;

impl StyleTokenSet for BuiltInStyleTokens {
    type Resolved = ();

    fn resolve(vocabulary: &mut StyleVocabulary) -> Self::Resolved {
        let mut bindings = vocabulary.id_bindings();

        bindings.pseudo_class(PSEUDO_HOVER, ":hover");
        bindings.pseudo_class(PSEUDO_PRESSED, ":pressed");
        bindings.pseudo_class(PSEUDO_FOCUSED, ":focused");
        bindings.pseudo_class(PSEUDO_DISABLED, ":disabled");
        bindings.pseudo_class(PSEUDO_CHECKED, ":checked");
        bindings.pseudo_class(PSEUDO_READ_ONLY, ":read-only");
        bindings.pseudo_class(PSEUDO_FOCUS_VISIBLE, ":focus-visible");
        bindings.pseudo_class(PSEUDO_SELECTED, ":selected");
        bindings.pseudo_class(PSEUDO_PLACEHOLDER_SHOWN, ":placeholder-shown");
        bindings.pseudo_class(PSEUDO_EXPANDED, ":expanded");

        bindings.class(CLASS_SLIDER_MAJOR_TICK, ".slider-major-tick");
        bindings.class(CLASS_SLIDER_MINOR_TICK, ".slider-minor-tick");

        bindings.resource(theme::SURFACE_BG, "surface.bg");
        bindings.resource(theme::ACCENT_BG, "accent.bg");
        bindings.resource(theme::ACCENT_HOVER_BG, "accent.hover.bg");
        bindings.resource(theme::ACCENT_PRESSED_BG, "accent.pressed.bg");
        bindings.resource(theme::SUCCESS_BG, "success.bg");
        bindings.resource(theme::TRACK_BG, "track.bg");
        bindings.resource(theme::THUMB_BG, "thumb.bg");
        bindings.resource(theme::ON_ACCENT_FG, "on-accent.fg");
        bindings.resource(theme::ON_SURFACE_MUTED_FG, "on-surface.muted.fg");
        bindings.resource(theme::FOCUS_RING_BORDER, "focus-ring.border");
        bindings.resource(theme::ON_SURFACE_FG, "on-surface.fg");
        bindings.resource(theme::PANEL_BG, "panel.bg");
        bindings.resource(theme::CONTROL_BG, "control.bg");
        bindings.resource(theme::CONTROL_BORDER, "control.border");
        bindings.resource(theme::CONTROL_HOVER_BORDER, "control.hover.border");
        bindings.resource(theme::CONTROL_HOVER_BG, "control.hover.bg");
        bindings.resource(theme::TEXT_CARET_BG, "text.caret.bg");
        bindings.resource(theme::TEXT_SELECTION_BG, "text.selection.bg");
        bindings.resource(theme::SLIDER_MAJOR_TICK_BG, "slider.major-tick.bg");
        bindings.resource(theme::SLIDER_MINOR_TICK_BG, "slider.minor-tick.bg");
        bindings.resource(theme::MOTION_FAST_DURATION, "motion.fast.duration");
        bindings.resource(theme::MOTION_NORMAL_DURATION, "motion.normal.duration");
        bindings.resource(
            theme::MOTION_EMPHASIZED_DURATION,
            "motion.emphasized.duration",
        );
        bindings.resource(theme::MOTION_STANDARD_EASING, "motion.standard.easing");
        bindings.resource(
            theme::MOTION_COLOR_INTERPOLATION,
            "motion.color-interpolation",
        );

        bindings.type_tag(TYPE_GENERIC, "Panel");
        bindings.type_tag(TYPE_ROOT, "Root");
        bindings.type_tag(TYPE_TEXT, "Text");
        bindings.type_tag(TYPE_BUTTON, "Button");
        bindings.type_tag(TYPE_TOGGLE, "ToggleSwitch");
        bindings.type_tag(TYPE_DIVIDER, "Divider");
        bindings.type_tag(TYPE_SPINNER, "Spinner");
        bindings.type_tag(TYPE_SLIDER, "Slider");
        bindings.type_tag(TYPE_PROGRESS_BAR, "ProgressBar");
        bindings.type_tag(TYPE_CHECK_BOX, "CheckBox");
        bindings.type_tag(TYPE_TEXT_INPUT, "TextInput");
        bindings.type_tag(TYPE_SPLITTER, "Splitter");
        bindings.type_tag(TYPE_LIST_BOX, "ListBox");
        bindings.type_tag(TYPE_LIST_ITEM, "ListItem");
        bindings.type_tag(TYPE_MENU_BUTTON, "MenuButton");
        bindings.type_tag(TYPE_MENU, "Menu");
        bindings.type_tag(TYPE_MENU_ITEM, "MenuItem");
        bindings.type_tag(TYPE_SCROLL_VIEW, "ScrollView");
        bindings.type_tag(TYPE_SCROLL_BAR, "ScrollBar");
        bindings.type_tag(TYPE_TOOLTIP, "Tooltip");
        bindings.type_tag(TYPE_POPUP, "Popup");
        bindings.type_tag(TYPE_VIEWPORT, "Viewport");
        bindings.type_tag(TYPE_SELECT, "Select");
        bindings.type_tag(TYPE_VECTOR_PATH, "VectorPath");
        bindings.type_tag(TYPE_SCROLL_AREA, "ScrollArea");
        bindings.type_tag(TYPE_OUTLINE_VIEW, "OutlineView");
        bindings.type_tag(TYPE_OUTLINE_ITEM, "OutlineItem");
        bindings.type_tag(TYPE_RADIO_BUTTON, "RadioButton");
        bindings.type_tag(TYPE_RADIO_GROUP, "RadioGroup");
        bindings.type_tag(TYPE_EXPANDER, "Expander");
        bindings.type_tag(TYPE_SEGMENTED_CONTROL, "SegmentedControl");
        bindings.type_tag(TYPE_SEGMENTED_ITEM, "SegmentedItem");
        bindings.type_tag(TYPE_ACCORDION, "Accordion");
        bindings.type_tag(TYPE_ACCORDION_SECTION, "AccordionSection");

        bindings.part(TYPE_BUTTON, PART_BUTTON_LABEL, "Button.Label");
        bindings.part(TYPE_TOGGLE, PART_TOGGLE_THUMB, "Toggle.Thumb");
        bindings.part(TYPE_SLIDER, PART_SLIDER_TRACK, "Slider.Track");
        bindings.part(TYPE_SLIDER, PART_SLIDER_THUMB, "Slider.Thumb");
        bindings.part(TYPE_SLIDER, PART_SLIDER_AXIS, "Slider.Axis");
        bindings.part(TYPE_SLIDER, PART_SLIDER_TICK, "Slider.Tick");
        bindings.part(TYPE_SLIDER, PART_SLIDER_TICK_LABEL, "Slider.TickLabel");
        bindings.part(
            TYPE_PROGRESS_BAR,
            PART_PROGRESS_BAR_FILL,
            "ProgressBar.Fill",
        );
        bindings.part(
            TYPE_PROGRESS_BAR,
            PART_PROGRESS_BAR_LABEL,
            "ProgressBar.Label",
        );
        bindings.part(TYPE_CHECK_BOX, PART_CHECK_BOX_BOX, "CheckBox.Box");
        bindings.part(TYPE_CHECK_BOX, PART_CHECK_BOX_MARK, "CheckBox.Mark");
        bindings.part(TYPE_CHECK_BOX, PART_CHECK_BOX_LABEL, "CheckBox.Label");
        bindings.part(TYPE_TEXT_INPUT, PART_TEXT_INPUT_TEXT, "TextInput.Text");
        bindings.part(TYPE_TEXT_INPUT, PART_TEXT_INPUT_CARET, "TextInput.Caret");
        bindings.part(
            TYPE_TEXT_INPUT,
            PART_TEXT_INPUT_SELECTION,
            "TextInput.Selection",
        );
        bindings.part(TYPE_LIST_ITEM, PART_LIST_ITEM_CONTENT, "ListItem.Content");
        bindings.part(TYPE_LIST_ITEM, PART_LIST_ITEM_LABEL, "ListItem.Label");
        bindings.part(TYPE_MENU_BUTTON, PART_MENU_BUTTON_LABEL, "MenuButton.Label");
        bindings.part(
            TYPE_MENU_BUTTON,
            PART_MENU_BUTTON_INDICATOR,
            "MenuButton.Indicator",
        );
        bindings.part(TYPE_MENU_ITEM, PART_MENU_ITEM_LABEL, "MenuItem.Label");
        bindings.part(
            TYPE_MENU_ITEM,
            PART_MENU_ITEM_INDICATOR,
            "MenuItem.Indicator",
        );
        bindings.part(TYPE_SCROLL_BAR, PART_SCROLL_BAR_THUMB, "ScrollBar.Thumb");
        bindings.part(TYPE_TOOLTIP, PART_TOOLTIP_LABEL, "Tooltip.Label");
        bindings.part(TYPE_SELECT, PART_SELECT_LABEL, "Select.Label");
        bindings.part(TYPE_SELECT, PART_SELECT_INDICATOR, "Select.Indicator");
        bindings.part(
            TYPE_OUTLINE_ITEM,
            PART_OUTLINE_ITEM_CONTENT,
            "OutlineItem.Content",
        );
        bindings.part(
            TYPE_OUTLINE_ITEM,
            PART_OUTLINE_ITEM_DISCLOSURE,
            "OutlineItem.Disclosure",
        );
        bindings.part(
            TYPE_OUTLINE_ITEM,
            PART_OUTLINE_ITEM_LABEL,
            "OutlineItem.Label",
        );
        bindings.part(
            TYPE_RADIO_BUTTON,
            PART_RADIO_BUTTON_INDICATOR,
            "RadioButton.Indicator",
        );
        bindings.part(TYPE_RADIO_BUTTON, PART_RADIO_BUTTON_DOT, "RadioButton.Dot");
        bindings.part(
            TYPE_RADIO_BUTTON,
            PART_RADIO_BUTTON_LABEL,
            "RadioButton.Label",
        );
        bindings.part(TYPE_EXPANDER, PART_EXPANDER_HEADER, "Expander.Header");
        bindings.part(
            TYPE_EXPANDER,
            PART_EXPANDER_DISCLOSURE,
            "Expander.Disclosure",
        );
        bindings.part(TYPE_EXPANDER, PART_EXPANDER_LABEL, "Expander.Label");
        bindings.part(TYPE_EXPANDER, PART_EXPANDER_CONTENT, "Expander.Content");
        bindings.part(
            TYPE_SEGMENTED_CONTROL,
            PART_SEGMENTED_SELECTION_INDICATOR,
            "SegmentedControl.SelectionIndicator",
        );
        bindings.part(
            TYPE_SEGMENTED_ITEM,
            PART_SEGMENTED_ITEM_LABEL,
            "SegmentedItem.Label",
        );
        bindings.part(
            TYPE_ACCORDION_SECTION,
            PART_ACCORDION_SECTION_HEADER,
            "AccordionSection.Header",
        );
        bindings.part(
            TYPE_ACCORDION_SECTION,
            PART_ACCORDION_SECTION_DISCLOSURE,
            "AccordionSection.Disclosure",
        );
        bindings.part(
            TYPE_ACCORDION_SECTION,
            PART_ACCORDION_SECTION_LABEL,
            "AccordionSection.Label",
        );
        bindings.part(
            TYPE_ACCORDION_SECTION,
            PART_ACCORDION_SECTION_CONTENT,
            "AccordionSection.Content",
        );
    }
}

impl Ui {
    /// Returns the human-readable style vocabulary for this UI.
    ///
    /// The vocabulary maps author-facing style names to the compact ids used by
    /// selectors and matching. Inspection snapshots carry a clone of this table
    /// so tools can show app-defined names such as `.demo-card`.
    #[must_use]
    pub fn style_vocabulary(&self) -> &StyleVocabulary {
        &self.style_vocabulary
    }

    /// Returns the class id for `name`, registering it when first encountered.
    pub fn class_id(&mut self, name: impl Into<String>) -> ClassId {
        self.style_vocabulary.class_id(name)
    }

    /// Returns the pseudoclass id for `name`, registering it when first encountered.
    pub fn pseudo_class_id(&mut self, name: impl Into<String>) -> PseudoClassId {
        self.style_vocabulary.pseudo_class_id(name)
    }

    /// Returns the type tag for `name`, registering it when first encountered.
    pub fn type_tag(&mut self, name: impl Into<String>) -> TypeTag {
        self.style_vocabulary.type_tag(name)
    }

    /// Returns the resource key for `name`, registering it when first encountered.
    pub fn resource_key(&mut self, name: impl Into<String>) -> ResourceKey {
        self.style_vocabulary.resource_key(name)
    }

    /// Returns the owner-local part tag for `name`, registering it when first encountered.
    pub fn part_tag(&mut self, owner: TypeTag, name: impl Into<String>) -> PartTag {
        self.style_vocabulary.part_tag(owner, name)
    }

    /// Resolves a reusable style token set against this UI's vocabulary.
    ///
    /// Downstream crates can implement [`StyleTokenSet`] on a marker type and
    /// call this once during setup to intern all names they own and receive the
    /// ids needed for selector construction.
    pub fn style_tokens<T>(&mut self) -> T::Resolved
    where
        T: StyleTokenSet,
    {
        self.style_vocabulary.style_tokens::<T>()
    }

    pub(crate) fn resolve_style_class(&mut self, class: impl Into<StyleClass>) -> ClassId {
        match class.into() {
            StyleClass::Id(id) => id,
            StyleClass::Name(name) => self.class_id(name),
        }
    }

    pub(in crate::ui) fn style_vocabulary_snapshot(&self) -> StyleVocabulary {
        self.style_vocabulary.clone()
    }

    pub(in crate::ui) fn register_builtin_style_vocabulary(&mut self) {
        self.style_tokens::<BuiltInStyleTokens>();
    }
}
