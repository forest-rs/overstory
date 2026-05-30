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
pub(super) fn demo_card(
    ui: &mut Ui,
    parent: ElementId,
    title: &str,
    width: f32,
    height: f32,
) -> ElementId {
    with_demo_card(ui, parent, title, width, height, |_| ()).0
}

pub(super) fn with_demo_card<R>(
    ui: &mut Ui,
    parent: ElementId,
    title: &str,
    width: f32,
    height: f32,
    f: impl FnOnce(&mut ChildUi<'_>) -> R,
) -> (ElementId, R) {
    ui.children(
        parent,
        Panel::column(14.0).class(CLASS_DEMO_CARD).layout(
            Layout::column(14.0)
                .logical_size(width, height)
                .padding_all(18.0),
        ),
        |card| {
            card.add(card_title(title));
            f(card)
        },
    )
}

pub(super) fn with_demo_fit_card<R>(
    ui: &mut Ui,
    parent: ElementId,
    title: &str,
    width: f32,
    min_height: f32,
    f: impl FnOnce(&mut ChildUi<'_>) -> R,
) -> (ElementId, R) {
    ui.children(
        parent,
        Panel::column(14.0).class(CLASS_DEMO_CARD).layout(
            Layout::column(14.0)
                .size(
                    LayoutSize::new()
                        .width(LayoutDimension::Logical(width))
                        .min_height(LayoutDimension::Logical(min_height)),
                )
                .padding_all(18.0),
        ),
        |card| {
            card.add(card_title(title));
            f(card)
        },
    )
}

pub(super) fn demo_row(ui: &mut Ui, parent: ElementId) -> ElementId {
    ui.append(
        parent,
        Panel::row(18.0).layout(
            Layout::row(18.0).wrap().size(
                LayoutSize::new()
                    .width(LayoutDimension::Percent(1.0))
                    .min_width(LayoutDimension::Logical(0.0)),
            ),
        ),
    )
}

/// Builds a click handler that cycles the button's label each time it fires.
pub(super) fn cycling_click_handler() -> impl FnMut(&mut Ui, overstory::ClickEvent<'_>) + 'static {
    let activations = Cell::new(0_u32);
    move |ui: &mut Ui, event| {
        if !matches!(
            event.phase,
            overstory::Phase::Target | overstory::Phase::Bubble
        ) {
            return;
        }
        let n = activations.get() + 1;
        activations.set(n);
        let label = if n == 1 {
            "Clicked!".to_owned()
        } else {
            format!("Clicked {n}×")
        };
        ui.set_local(event.current, ui.properties().content, label);
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
            ui.append(popup, Text::new("Review queue").font_size(13.0));
            ui.append(popup, Text::new("3 items pending").font_size(13.0));
        });
        open_popup.set(popup);
    }
}
