// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

pub(super) fn caption(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_CAPTION)
}

pub(super) fn heading(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_HEADING)
}

pub(super) fn page_heading(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_PAGE_HEADING)
}

pub(super) fn card_title(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_CARD_TITLE)
}

pub(super) fn control_label(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_CONTROL_LABEL)
}

pub(super) fn number_text(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_NUMBER)
}

pub(super) fn large_number_text(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_LARGE_NUMBER)
}

pub(super) fn body_text(text: &str) -> Text {
    Text::new(text).class(CLASS_DEMO_BODY).wrap()
}

pub(super) fn opt_solid(r: u8, g: u8, b: u8, a: u8) -> Option<Brush> {
    Some(solid(r, g, b, a))
}

pub(super) fn solid(r: u8, g: u8, b: u8, a: u8) -> Brush {
    Color::from_rgba8(r, g, b, a).into()
}

pub(super) fn tabular_number_features() -> Box<[FontFeature]> {
    let feature = FontFeature::parse_css_list("\"tnum\" on")
        .next()
        .expect("tabular-number feature should parse")
        .expect("tabular-number feature should be valid");
    Box::from([feature])
}

pub(super) trait DemoChildUiExt {
    fn demo_auto_card<R>(
        &mut self,
        title: &str,
        width: f32,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> R;

    fn demo_card<R>(
        &mut self,
        title: &str,
        width: f32,
        height: f32,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> R;

    /// A fluid card: `min_width` is the floor, but the card grows to share the
    /// row and wraps to the next line when the row runs out of space. Height
    /// follows content. Use inside a `Layout::row(..).wrap().fill_width()` so
    /// cards reflow into a responsive grid instead of a fixed pixel layout.
    fn demo_responsive_card<R>(
        &mut self,
        title: &str,
        min_width: f32,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> R;
}

impl DemoChildUiExt for ChildUi<'_> {
    fn demo_auto_card<R>(
        &mut self,
        title: &str,
        width: f32,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> R {
        self.with_child(
            Panel::column(14.0)
                .class(CLASS_DEMO_CARD)
                .padding_all(18.0)
                .layout(
                    Layout::column(14.0).size(
                        LayoutSize::new()
                            .width(LayoutDimension::Logical(width))
                            .min_width(LayoutDimension::Logical(0.0)),
                    ),
                ),
            |card| {
                card.add(card_title(title));
                f(card)
            },
        )
    }

    fn demo_card<R>(
        &mut self,
        title: &str,
        width: f32,
        height: f32,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> R {
        self.with_child(
            Panel::column(14.0)
                .class(CLASS_DEMO_CARD)
                .padding_all(18.0)
                .layout(Layout::column(14.0).logical_size(width, height)),
            |card| {
                card.add(card_title(title));
                f(card)
            },
        )
    }

    fn demo_responsive_card<R>(
        &mut self,
        title: &str,
        min_width: f32,
        f: impl FnOnce(&mut ChildUi<'_>) -> R,
    ) -> R {
        self.with_child(
            Panel::column(14.0)
                .class(CLASS_DEMO_CARD)
                .padding_all(18.0)
                .layout(
                    Layout::column(14.0)
                        .size(
                            LayoutSize::new()
                                .width(LayoutDimension::Logical(min_width))
                                .min_width(LayoutDimension::Logical(min_width)),
                        )
                        .flex_grow(1.0)
                        .flex_shrink(1.0),
                ),
            |card| {
                card.add(card_title(title));
                f(card)
            },
        )
    }
}

/// Builds a click handler that cycles the button's label each time it fires.
pub(super) fn cycling_click_handler(
    activations: LocalState<u32>,
) -> impl FnMut(&mut Ui, overstory::ClickEvent<'_>) + 'static {
    move |ui: &mut Ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        activations.update(ui, |count| *count += 1);
    }
}

pub(super) fn cycling_click_label(initial: &str, activations: u32) -> String {
    if activations == 0 {
        initial.to_owned()
    } else if activations == 1 {
        "Clicked!".to_owned()
    } else {
        format!("Clicked {activations}×")
    }
}

pub(super) fn popup_click_handler() -> impl FnMut(&mut Ui, overstory::ClickEvent<'_>) + 'static {
    let open_popup = Cell::new(None);
    move |ui: &mut Ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        if let Some(popup) = open_popup.get()
            && ui.close_popup(popup)
        {
            open_popup.set(None);
            return;
        }
        let popup = ui.open_popup(event.current, Popup::new(), |ui, popup| {
            ui.with_children(popup, |popup| {
                popup.add(Text::new("Review queue").font_size(13.0));
                popup.add(Text::new("3 items pending").font_size(13.0));
            });
        });
        open_popup.set(popup);
    }
}
