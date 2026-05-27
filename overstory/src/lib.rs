// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]

//! Overstory: a WPF / WinUI 3-inspired UI toolkit built on
//! `understory` and `imaging`.
//!
//! Each control is a struct that implements [`AppendSpec`] (the
//! builder + entry point) and produces a [`Widget`] instance that
//! lives on the element. Application code uses
//! [`Ui::append`](crate::Ui::append) to insert controls into the
//! tree:
//!
//! ```ignore
//! ui.append(parent, Button::new("Click me").on_click(|_, _| {}));
//! ui.append(parent, ToggleSwitch::new().checked(true));
//! ui.append(parent, Slider::new().value(0.4));
//! ```
//!
//! `Ui::append` is the only public path for adding controls;
//! `insert_child` lives on the toolkit-internal side for templates
//! building their own parts. `set_local` remains public for power
//! users (bindings, dynamic theming, custom controls), but isn't
//! the typical app-level entry point.
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

mod behavior;
mod binding_runtime;
pub mod channels;
mod composition;
mod controls;
mod element;
mod element_side_map;
mod element_store;
mod event_runtime;
mod focus_runtime;
mod generic;
mod geometry_runtime;
mod layout_runtime;
pub mod layouts;
mod lookup;
pub mod motion;
mod overlay;
mod presentation_runtime;
mod properties;
pub mod pseudos;
pub mod selectors;
mod style_runtime;
mod template;
mod template_runtime;
#[cfg(test)]
pub(crate) mod test_support;
mod text;
pub mod theme;
mod timeline_properties;
mod timeline_runtime;
mod timer_runtime;
mod ui;
mod widget;

pub use behavior::{
    KnownDimensions, LayoutContext, MeasureConstraints, MeasureContext, PresentContext,
    WidgetEventContext,
};
pub use composition::{CompositionPolicy, CompositionSurface, SurfaceId};
pub use controls::button::{Button, PART_BUTTON_LABEL, SLOT_BUTTON_CONTENT, TYPE_BUTTON};
pub use controls::checkbox::{
    CheckBox, PART_CHECK_BOX_BOX, PART_CHECK_BOX_LABEL, PART_CHECK_BOX_MARK, SLOT_CHECK_BOX_BOX,
    SLOT_CHECK_BOX_LABEL, SLOT_CHECK_BOX_MARK, TYPE_CHECK_BOX, set_check_box_checked,
};
pub use controls::divider::{Divider, DividerAxis, TYPE_DIVIDER};
pub use controls::list::{
    ListBox, ListSelectionChangedEvent, ListSelectionMode, PART_LIST_ITEM_LABEL,
    SLOT_LIST_ITEM_CONTENT, TYPE_LIST_BOX, TYPE_LIST_ITEM, list_scroll_offset_y,
    realized_list_item_index, scroll_list_index_into_view, select_list_index, selected_list_index,
    selected_list_indices, set_list_scroll_offset_y,
};
pub use controls::panel::Panel;
pub use controls::popup::{Popup, TYPE_POPUP};
pub use controls::progress::{
    PART_PROGRESS_BAR_FILL, PART_PROGRESS_BAR_LABEL, ProgressBar, SLOT_PROGRESS_BAR_FILL,
    SLOT_PROGRESS_BAR_LABEL, TYPE_PROGRESS_BAR, set_progress_value,
};
pub use controls::scroll::{
    PART_SCROLL_BAR_THUMB, SLOT_SCROLL_BAR_THUMB, ScrollBar, ScrollState, ScrollView,
    TYPE_SCROLL_BAR, TYPE_SCROLL_VIEW, scroll_view_offset_y, scroll_view_state,
    set_scroll_view_offset_y,
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
pub use controls::text::{TYPE_TEXT, Text, TextWidget};
pub use controls::text_input::{
    PART_TEXT_INPUT_CARET, PART_TEXT_INPUT_SELECTION, PART_TEXT_INPUT_TEXT, SLOT_TEXT_INPUT_CARET,
    SLOT_TEXT_INPUT_SELECTION, SLOT_TEXT_INPUT_TEXT, TYPE_TEXT_INPUT, TextInput,
    set_text_input_read_only,
};
pub use controls::toggle::{
    PART_TOGGLE_THUMB, SLOT_TOGGLE_THUMB, TYPE_TOGGLE, ToggleSwitch, toggle_checked,
};
pub use controls::tooltip::{PART_TOOLTIP_LABEL, SLOT_TOOLTIP_CONTENT, TYPE_TOOLTIP, Tooltip};
pub use controls::viewport::{TYPE_VIEWPORT, Viewport};
pub use cursor_icon::CursorIcon;
pub use element::{Element, ElementId};
pub use event_runtime::{
    ClickEvent, DragEvent, FocusChangeReason, FocusEvent, KeyDispatchResult, KeyEvent,
    PointerDispatchResult, PointerEventKind, PointerRoutedEvent, RoutedDispatchResult,
    RoutedEventContext, RoutedEventKind, RoutedHandlerOptions, ScrollEvent,
};
pub use generic::present_surface;
pub use layouts::Layout;
pub use lookup::{UiResolveLookup, UiStoreLookup};
pub use overlay::Placement;
pub use properties::BuiltInProperties;
pub use template::{
    Template, TemplateBinding, TemplateBuiltInProperty, TemplateChild, TemplateContext,
    TemplateInstance, TemplateItemKey, TemplateNode, TemplatePart, TemplateSlot, TemplateSpec,
    TemplateWidgetFactory,
};
pub use text::{TextEngine, TextLayout, TextLayoutCacheStats, TextResources};
pub use timeline_runtime::{TimelineCue, TimelineCueId};
pub use timer_runtime::TimerEvent;
pub use ui::{ControlState, Ui};
pub use widget::{AppendSpec, TYPE_GENERIC, Widget};

// Re-export the responder's `Phase` so callers can read
// `ClickEvent::phase` / `KeyEvent::phase` etc. without depending
// on `understory_responder` directly.
pub use understory_focus::Navigation;
pub use understory_presentation::{
    FontFamily, FontFamilyName, FontStyle, FontWeight, FontWidth, GenericFamily, Language,
    OverflowWrap, TextLineHeight, TextWrapMode, WordBreak,
};
pub use understory_responder::types::Phase;
pub use understory_timing::{TimerDuration, TimerId, TimerInstant};
