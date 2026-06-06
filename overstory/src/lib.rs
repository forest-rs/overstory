// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]

//! Overstory: a WPF / WinUI 3-inspired UI toolkit built on
//! `understory` and `imaging`.
//!
//! Each control is a struct that implements [`AppendSpec`] (the
//! builder + entry point) and produces a [`Widget`] instance that
//! lives on the element. Application code can use [`Ui::with_children`],
//! [`ChildUi::with_child`], and [`ChildUi::add`] to keep parentage lexical while
//! preserving the same retained tree machinery:
//!
//! ```ignore
//! ui.with_child(root, Panel::column(12.0), |ui| {
//!     ui.add(Text::new("Inputs"));
//!     ui.add(Button::new("Apply").on_click(|_, _| {}));
//!     ui.with_child(Panel::row(8.0), |row| {
//!         row.add(ToggleSwitch::new().checked(true));
//!         row.add(Text::new("Live rendering"));
//!     });
//! });
//! ```
//!
//! Reusable app sections can implement [`View`] and build through the same
//! scoped child API. A view is an authoring component, not a runtime widget.
//! Import [`prelude`] for normal app code, or use [`authoring`] for focused
//! imports of the shared tree/spec/binding extension traits.
//!
//! [`Ui::append`] remains the primitive explicit-parent path; `insert_child`
//! lives on the toolkit-internal side for templates building their own parts.
//! `set_local` remains public for power users (bindings, dynamic theming,
//! custom controls), but isn't the typical app-level entry point.
//!
//! Roadmap:
//! - Keep `overstory` itself as the no-std core toolkit: retained UI tree,
//!   controls, styling, layout intent, routed events, presentation lowering,
//!   and compositor-neutral composition data.
//! - Keep platform integration in host crates. Windowing, GPU setup, clipboard,
//!   IME, accessibility bridges, frame pacing, and native compositor surfaces
//!   should not leak into core controls.
//! - Grow public APIs from repeated control needs, not from mirroring WPF,
//!   WinUI, Taffy, Subduction, Parley, or any host backend wholesale.
//! - Make diagnostics first-class: invalidation, layout, text shaping,
//!   composition, timers, and event dispatch should all become measurable.
//! - Preserve one obvious path for app authors: construct controls, bind or
//!   style properties, handle routed events, and let the frame pipeline do the
//!   rest.
//!
//! Detailed roadmap tickets live under `.tickets/`; see epic `ove-p3tl`.

#[cfg(any(feature = "std", test))]
extern crate std;

#[cfg_attr(test, macro_use)]
extern crate alloc;

pub mod accessibility;
pub mod authoring;
mod behavior;
mod binding;
mod binding_runtime;
pub mod channels;
mod composition;
mod controls;
pub mod diagnostics;
mod element;
mod element_ref;
mod element_side_map;
mod element_store;
mod event_runtime;
mod focus_runtime;
mod generic;
mod geometry_runtime;
pub mod inspection;
mod inspection_runtime;
mod layout_runtime;
pub mod layouts;
mod lookup;
pub mod motion;
mod overlay;
mod presentation_runtime;
mod properties;
pub mod pseudos;
mod scroll_runtime;
pub mod selectors;
mod spec_ext;
mod style_runtime;
mod template;
#[cfg(test)]
pub(crate) mod test_support;
mod text;
pub mod theme;
mod timeline_properties;
mod timeline_runtime;
mod timer_runtime;
mod ui;
mod view;
mod widget;

pub use accessibility::AccessibilityContext;
pub use accesskit;
pub use behavior::{
    ArrangeContext, ArrangeResult, ElementRealizeContext, ElementRealizeResult, KnownDimensions,
    MeasureConstraints, MeasureContext, PresentContext, WidgetEventContext,
};
pub use binding::{
    BindingOwner, BindingSpecExt, BoundSpec, ExternalSource, ExternalSourceId, LocalState,
    LocalStateId, ValueBinding,
};
pub use composition::{CompositionPolicy, CompositionSurface, SurfaceId};
pub use controls::accordion::{
    Accordion, AccordionChildUiExt, AccordionExpandedChangedEvent, AccordionMode, AccordionSection,
    PART_ACCORDION_SECTION_CONTENT, PART_ACCORDION_SECTION_DISCLOSURE,
    PART_ACCORDION_SECTION_HEADER, PART_ACCORDION_SECTION_LABEL, SLOT_ACCORDION_SECTION_CONTENT,
    SLOT_ACCORDION_SECTION_DISCLOSURE, SLOT_ACCORDION_SECTION_HEADER, SLOT_ACCORDION_SECTION_LABEL,
    TYPE_ACCORDION, TYPE_ACCORDION_SECTION, accordion_section_content, accordion_section_expanded,
    set_accordion_section_expanded, toggle_accordion_section,
};
pub use controls::button::{Button, PART_BUTTON_LABEL, SLOT_BUTTON_CONTENT, TYPE_BUTTON};
pub use controls::checkbox::{
    CheckBox, PART_CHECK_BOX_BOX, PART_CHECK_BOX_LABEL, PART_CHECK_BOX_MARK, SLOT_CHECK_BOX_BOX,
    SLOT_CHECK_BOX_LABEL, SLOT_CHECK_BOX_MARK, TYPE_CHECK_BOX, set_check_box_checked,
};
pub use controls::choice::ChoiceOption;
pub use controls::divider::{Divider, TYPE_DIVIDER};
pub use controls::expander::{
    ExpandedChangedEvent, Expander, PART_EXPANDER_CONTENT, PART_EXPANDER_DISCLOSURE,
    PART_EXPANDER_HEADER, PART_EXPANDER_LABEL, SLOT_EXPANDER_CONTENT, SLOT_EXPANDER_DISCLOSURE,
    SLOT_EXPANDER_HEADER, SLOT_EXPANDER_LABEL, TYPE_EXPANDER, expander_content, expander_expanded,
    set_expander_expanded, toggle_expander,
};
pub use controls::list::{
    ListBox, ListBoxScrollArea, ListItemContext, ListSelectionChangedEvent, ListSelectionMode,
    PART_LIST_ITEM_CONTENT, PART_LIST_ITEM_LABEL, SLOT_LIST_ITEM_CONTENT, TYPE_LIST_BOX,
    TYPE_LIST_ITEM, list_box_scroll_area_list, list_box_scroll_area_scrollbar,
    list_scroll_offset_y, realized_list_item_index, scroll_list_index_into_view, select_list_index,
    selected_list_index, selected_list_indices, set_list_scroll_offset_y,
};
pub use controls::outline::{
    OutlineExpansionChangedEvent, OutlineItemInfo, OutlineNodeKey, OutlineSelectionChangedEvent,
    OutlineView, OutlineViewNode, OutlineViewScrollArea, PART_OUTLINE_ITEM_CONTENT,
    PART_OUTLINE_ITEM_DISCLOSURE, PART_OUTLINE_ITEM_LABEL, SLOT_OUTLINE_ITEM_DISCLOSURE,
    SLOT_OUTLINE_ITEM_LABEL, TYPE_OUTLINE_ITEM, TYPE_OUTLINE_VIEW, expanded_outline_keys,
    outline_key_expanded, outline_scroll_offset_y, outline_view_scroll_area_outline,
    outline_view_scroll_area_scrollbar, realized_outline_item_info, realized_outline_item_key,
    replace_outline_nodes, select_outline_key, selected_outline_key, set_outline_key_expanded,
    set_outline_scroll_offset_y, toggle_outline_key,
};
pub use controls::panel::Panel;
pub use controls::popup::{Popup, TYPE_POPUP};
pub use controls::progress::{
    PART_PROGRESS_BAR_FILL, PART_PROGRESS_BAR_LABEL, ProgressBar, SLOT_PROGRESS_BAR_FILL,
    SLOT_PROGRESS_BAR_LABEL, TYPE_PROGRESS_BAR, set_progress_value,
};
pub use controls::radio::{
    PART_RADIO_BUTTON_DOT, PART_RADIO_BUTTON_INDICATOR, PART_RADIO_BUTTON_LABEL, RadioButton,
    RadioGroup, RadioSelectionChangedEvent, SLOT_RADIO_BUTTON_DOT, SLOT_RADIO_BUTTON_INDICATOR,
    SLOT_RADIO_BUTTON_LABEL, TYPE_RADIO_BUTTON, TYPE_RADIO_GROUP, clear_radio_group_selection,
    radio_button_checked, selected_radio_group_index, selected_radio_group_item,
    set_radio_button_checked, set_radio_group_selected_index,
};
pub use controls::scroll::{
    PART_SCROLL_BAR_THUMB, SLOT_SCROLL_BAR_THUMB, ScrollArea, ScrollBar, ScrollView,
    TYPE_SCROLL_AREA, TYPE_SCROLL_BAR, TYPE_SCROLL_VIEW, scroll_area_horizontal_bar,
    scroll_area_vertical_bar, scroll_area_viewport, scroll_offset_y, scroll_view_offset_y,
    set_scroll_offset_y, set_scroll_view_offset_y,
};
pub use controls::segmented::{
    PART_SEGMENTED_ITEM_LABEL, SLOT_SEGMENTED_ITEM_LABEL, SegmentedControl,
    SegmentedSelectionChangedEvent, TYPE_SEGMENTED_CONTROL, TYPE_SEGMENTED_ITEM,
    clear_segmented_selection, segmented_item, selected_segmented_index, selected_segmented_item,
    set_segmented_index,
};
pub use controls::select::{
    PART_SELECT_INDICATOR, PART_SELECT_LABEL, SLOT_SELECT_INDICATOR, SLOT_SELECT_LABEL, Select,
    SelectSelectionChangedEvent, TYPE_SELECT, clear_select_selection, selected_select_index,
    selected_select_item, set_select_index,
};
pub use controls::slider::{
    CLASS_SLIDER_MAJOR_TICK, CLASS_SLIDER_MINOR_TICK, PART_SLIDER_AXIS, PART_SLIDER_THUMB,
    PART_SLIDER_TICK, PART_SLIDER_TICK_LABEL, PART_SLIDER_TRACK, SLOT_SLIDER_AXIS,
    SLOT_SLIDER_THUMB, SLOT_SLIDER_TICK, SLOT_SLIDER_TICK_LABEL, SLOT_SLIDER_TRACK, Slider,
    TYPE_SLIDER, set_slider_value,
};
pub use controls::spinner::{Spinner, TYPE_SPINNER};
pub use controls::splitter::{
    Splitter, SplitterOrientation, SplitterResizeEvent, TYPE_SPLITTER, resize_split_pair,
};
pub use controls::support::CheckedChangedEvent;
pub use controls::text::{TYPE_TEXT, Text, TextWidget};
pub use controls::text_input::{
    PART_TEXT_INPUT_CARET, PART_TEXT_INPUT_SELECTION, PART_TEXT_INPUT_TEXT, SLOT_TEXT_INPUT_CARET,
    SLOT_TEXT_INPUT_SELECTION, SLOT_TEXT_INPUT_TEXT, TYPE_TEXT_INPUT, TextInput,
    TextInputSubmitEvent, set_text_input_read_only, set_text_input_text, text_input_text,
};
pub use controls::toggle::{
    PART_TOGGLE_THUMB, SLOT_TOGGLE_THUMB, TYPE_TOGGLE, ToggleSwitch, toggle_checked,
};
pub use controls::tooltip::{
    PART_TOOLTIP_LABEL, SLOT_TOOLTIP_CONTENT, TYPE_TOOLTIP, Tooltip, TooltipSpec, TooltipSpecExt,
};
pub use controls::vector_path::{TYPE_VECTOR_PATH, VectorPath};
pub use controls::viewport::{TYPE_VIEWPORT, Viewport};
pub use cursor_icon::CursorIcon;
pub use diagnostics::{ChannelMarkStats, UiFrameStats, UiFrameTimings, UiRendererTimings};
pub use element::{Element, ElementId};
pub use element_ref::{ElementRef, ElementRefSpecExt, ReferencedSpec};
pub use event_runtime::{
    ClickEvent, DragEvent, FocusChangeReason, FocusEvent, KeyDispatchResult, KeyEvent,
    PointerDispatchResult, PointerEventKind, PointerRoutedEvent, RoutedDispatchResult,
    RoutedEventContext, RoutedEventKind, RoutedEventOrigin, RoutedHandlerOptions,
    RoutedHandlerScope, ScrollEvent,
};
pub use generic::present_surface;
pub use layouts::{
    Layout, LayoutAlign, LayoutAlignment, LayoutContainer, LayoutDimension, LayoutDirection,
    LayoutDisplay, LayoutEdges, LayoutGap, LayoutJustify, LayoutOverflow, LayoutOverflowMode,
    LayoutPlacement, LayoutSize, LayoutSpacing, LayoutWrap,
};
pub use lookup::{UiResolveLookup, UiStoreLookup};
pub use overlay::Placement;
pub use properties::BuiltInProperties;
pub use scroll_runtime::ScrollState;
pub use template::{
    ControlTemplate, Template, TemplateChild, TemplateContext, TemplateContract,
    TemplateDiagnostic, TemplateExpansionReport, TemplateId, TemplateInstance, TemplateItemKey,
    TemplateNode, TemplatePart, TemplateSlot, TemplateSlotContract, TemplateSlotImportance,
    TemplateSlotStatus, TemplateSpec, TemplateSpecRecipe, TemplateWidgetFactory,
};
pub use text::{TextEngine, TextLayout, TextLayoutCacheStats, TextResources};
pub use timeline_runtime::{TimelineCue, TimelineCueId};
pub use timer_runtime::TimerEvent;
pub use ui::{ChildUi, ControlState, Ui};
pub use view::View;
pub use widget::{
    AppendSpec, ContentSlotSpec, ControlStateSpecExt, ControlTemplateSpec, ControlTemplateSpecExt,
    ElementConfig, ElementSpec, ElementSpecExt, RangeSpecExt, SurfaceSpecExt, SurfaceStyleExt,
    TYPE_GENERIC, TextSpecExt, Widget,
};

/// Common imports for authoring Overstory UI trees.
///
/// The prelude is intentionally app-author focused: controls, layout/value
/// types, the scoped child builder, and extension traits that make shared
/// element configuration methods available.
pub mod prelude {
    pub use crate::{
        Accordion, AccordionChildUiExt, AccordionMode, AccordionSection, AppendSpec,
        BindingSpecExt, Button, CheckBox, ChildUi, Color, ContentSlotSpec, ControlStateSpecExt,
        ControlTemplate, ControlTemplateSpec, ControlTemplateSpecExt, CursorIcon, Divider,
        ElementId, ElementRef, ElementRefSpecExt, ElementSpec, ElementSpecExt, Expander,
        ExternalSource, Layout, ListBox, ListItemContext, LocalState, OutlineView, Panel, Popup,
        ProgressBar, RadioButton, RadioGroup, RangeSpecExt, ScrollArea, ScrollBar, ScrollState,
        ScrollView, SegmentedControl, Select, Size, Slider, Spinner, Splitter, SurfaceSpecExt,
        SurfaceStyleExt, Text, TextInput, TextInputSubmitEvent, TextSpecExt, TimerDuration,
        ToggleSwitch, Tooltip, TooltipSpecExt, Ui, ValueBinding, VectorPath, View, Viewport,
    };
}

// Re-export the responder's `Phase` so callers can read
// `ClickEvent::phase` / `KeyEvent::phase` etc. without depending
// on `understory_responder` directly.
pub use parley::{FontFeature, FontVariation};
pub use peniko::kurbo::{Affine, BezPath, Cap, Join, Rect, Size, Stroke, Vec2};
pub use peniko::{Brush, Color, Fill, color::Rgba8};
pub use understory_focus::Navigation;
pub use understory_presentation::{
    CornerShape, CornerShapes, Corners, Edges, FontFamily, FontFamilyName, FontStyle, FontWeight,
    FontWidth, GenericFamily, Language, OverflowWrap, PathFill, PathPaintOrder, PathPrimitive,
    PathStroke, TextLineHeight, TextWrapMode, WordBreak,
};
pub use understory_presentation_properties::CornerRadius;
pub use understory_responder::types::Phase;
pub use understory_timing::{TimerDuration, TimerId, TimerInstant};
