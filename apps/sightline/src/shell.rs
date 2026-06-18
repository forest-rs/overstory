// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Sightline shell chrome.
//!
//! This module owns the baseline workspace shell and app-level controls. It
//! explicitly does not own telemetry transport or instrument-specific data
//! models.

use alloc::rc::Rc;
use alloc::string::ToString;

use overstory::{
    Button, ChildUi, ChoiceOption, ElementId, ElementSpecExt, Layout, LayoutAlign, LayoutAlignment,
    LayoutDimension, LayoutSize, Panel, Select, SurfaceSpecExt, Text, Ui,
};

use crate::app::AppState;
use crate::sample_profiler::{self, SampleProfilerView};
use crate::style::{SightlineTokens, sightline_theme};

const SHELL_PADDING: f32 = 18.0;
const TOP_BAR_HEIGHT: f32 = 60.0;

#[derive(Clone, Debug)]
pub(crate) struct Shell {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "source select handle is kept for focused pointer-route regressions"
        )
    )]
    source_select: ElementId,
    theme_button: ElementId,
    sample_profiler: SampleProfilerView,
}

#[derive(Clone, Copy, Debug)]
struct TopBar {
    source_select: ElementId,
    theme_button: ElementId,
}

pub(crate) fn build(
    ui: &mut Ui,
    root: ElementId,
    tokens: SightlineTokens,
    state: Rc<AppState>,
) -> Shell {
    let top_bar = ui.with_child(
        root,
        Panel::column(0.0)
            .class(tokens.shell)
            .layout(Layout::column(0.0).fill_size())
            .padding(SHELL_PADDING),
        |shell| {
            let top_bar = build_top_bar(shell, tokens, &state);
            let sample_profiler = build_workspace(shell, tokens, &state);
            (top_bar, sample_profiler)
        },
    );
    let (top_bar, sample_profiler) = top_bar;
    Shell {
        source_select: top_bar.source_select,
        theme_button: top_bar.theme_button,
        sample_profiler,
    }
}

pub(crate) fn wire(ui: &mut Ui, shell: Shell, tokens: SightlineTokens, state: Rc<AppState>) {
    let theme_state = Rc::clone(&state);
    ui.on_click(shell.theme_button, move |ui, event| {
        let next = theme_state.toggle_theme();
        ui.set_theme(sightline_theme(next, tokens));
        ui.set_local(
            event.semantic_source(),
            ui.properties().content,
            theme_button_label(next).to_string(),
        );
    });
    sample_profiler::wire(ui, shell.sample_profiler, state);
}

fn build_top_bar(shell: &mut ChildUi<'_>, tokens: SightlineTokens, state: &AppState) -> TopBar {
    shell.with_child(
        Panel::row(14.0)
            .class(tokens.top_bar)
            .layout(
                Layout::row(14.0)
                    .fill_width()
                    .size(
                        LayoutSize::new()
                            .height(LayoutDimension::Logical(TOP_BAR_HEIGHT))
                            .flex_shrink(0.0),
                    )
                    .alignment(LayoutAlignment::new().align_items(LayoutAlign::Center)),
            )
            .padding(14.0)
            .border_width(1.0)
            .corner_radius(8.0),
        |bar| {
            bar.add(
                Text::new("Sightline")
                    .font_size(22.0)
                    .class(tokens.title_text)
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            let source_select = bar.add(
                Select::new()
                    .items([
                        ChoiceOption::new("Local samples"),
                        ChoiceOption::new("Discover Spoor sessions"),
                        ChoiceOption::new("Open capture"),
                    ])
                    .selected_index(0)
                    .width(260.0),
            );
            bar.add(
                Text::new("ready")
                    .font_size(13.0)
                    .class(tokens.success_text)
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            bar.add(Panel::new().layout(Layout::new().fill_width()));
            let theme_button =
                bar.add(Button::new(theme_button_label(state.theme_mode())).font_size(13.0));
            TopBar {
                source_select,
                theme_button,
            }
        },
    )
}

fn build_workspace(
    shell: &mut ChildUi<'_>,
    tokens: SightlineTokens,
    state: &AppState,
) -> SampleProfilerView {
    shell
        .children(
            Panel::column(18.0)
                .class(tokens.workspace)
                .layout(Layout::column(18.0).fill_size()),
            |workspace| build_stage(workspace, tokens, state),
        )
        .1
}

fn build_stage(
    parent: &mut ChildUi<'_>,
    tokens: SightlineTokens,
    state: &AppState,
) -> SampleProfilerView {
    parent.with_child(
        Panel::column(18.0)
            .class(tokens.workspace)
            .layout(Layout::column(18.0).fill_size()),
        |stage| {
            stage.add(
                Text::new("Diagnostics Workspace")
                    .font_size(34.0)
                    .class(tokens.title_text)
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            stage.add(
                Text::new("Local sampled frame data")
                    .font_size(16.0)
                    .class(tokens.secondary_text)
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            sample_profiler::build(stage, tokens, state.sample_profiler())
        },
    )
}

fn theme_button_label(mode: crate::style::SightlineThemeMode) -> &'static str {
    mode.toggled().label()
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;

    use overstory::{Size, Ui};
    use peniko::kurbo::Point;
    use ui_events::pointer::{
        PointerButton, PointerButtonEvent, PointerButtons, PointerEvent, PointerId, PointerInfo,
        PointerState, PointerType,
    };

    use super::*;
    use crate::style::{SightlineStyleTokens, SightlineThemeMode, sightline_cascade};

    #[test]
    fn theme_button_switches_theme_and_updates_label() {
        let mut ui = Ui::new();
        let tokens = ui.style_tokens::<SightlineStyleTokens>();
        let state = Rc::new(AppState::new());
        ui.set_theme(sightline_theme(state.theme_mode(), tokens));
        ui.set_cascade(sightline_cascade(ui.properties(), tokens));

        let root = ui.root();
        let shell = build(&mut ui, root, tokens, Rc::clone(&state));
        wire(&mut ui, shell.clone(), tokens, Rc::clone(&state));

        assert_eq!(state.theme_mode(), SightlineThemeMode::Graphite);
        assert_eq!(theme_button_label(state.theme_mode()), "Blueprint");

        let _ = ui.synthesize_click(shell.theme_button, Point::ZERO);

        assert_eq!(state.theme_mode(), SightlineThemeMode::Blueprint);
        assert_eq!(theme_button_label(state.theme_mode()), "Graphite");
    }

    #[test]
    fn repeated_source_select_click_does_not_block_theme_button_pointer_click() {
        let mut ui = Ui::new();
        let tokens = ui.style_tokens::<SightlineStyleTokens>();
        let state = Rc::new(AppState::new());
        ui.set_theme(sightline_theme(state.theme_mode(), tokens));
        ui.set_cascade(sightline_cascade(ui.properties(), tokens));

        let root = ui.root();
        ui.set_layout(root, Layout::column(0.0).fill_size());
        let shell = build(&mut ui, root, tokens, Rc::clone(&state));
        wire(&mut ui, shell.clone(), tokens, Rc::clone(&state));
        ui.compute_layout(Size::new(1440.0, 900.0));

        let source_center = ui.effective_bounds(shell.source_select).center();
        pointer_click(&mut ui, source_center, 1);
        ui.compute_layout(Size::new(1440.0, 900.0));
        pointer_click(&mut ui, source_center, 3);
        ui.compute_layout(Size::new(1440.0, 900.0));

        let theme_center = ui.effective_bounds(shell.theme_button).center();
        assert_eq!(ui.pointer_capture(), None);
        pointer_click(&mut ui, theme_center, 5);

        assert_eq!(state.theme_mode(), SightlineThemeMode::Blueprint);
    }

    fn pointer_click(ui: &mut Ui, point: Point, time: u64) {
        let _ = ui.dispatch_pointer_event(&pointer_down(point, time));
        let _ = ui.dispatch_pointer_event(&pointer_up(point, time + 1));
    }

    fn pointer_down(point: Point, time: u64) -> PointerEvent {
        PointerEvent::Down(PointerButtonEvent {
            pointer: pointer_info(),
            button: Some(PointerButton::Primary),
            state: pointer_state(point, time, true),
        })
    }

    fn pointer_up(point: Point, time: u64) -> PointerEvent {
        PointerEvent::Up(PointerButtonEvent {
            pointer: pointer_info(),
            button: Some(PointerButton::Primary),
            state: pointer_state(point, time, false),
        })
    }

    fn pointer_info() -> PointerInfo {
        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn pointer_state(point: Point, time: u64, primary_down: bool) -> PointerState {
        let mut buttons = PointerButtons::new();
        if primary_down {
            buttons.insert(PointerButton::Primary);
        }
        let mut state = PointerState {
            time,
            buttons,
            count: 1,
            pressure: if primary_down { 0.5 } else { 0.0 },
            ..PointerState::default()
        };
        state.position.x = point.x;
        state.position.y = point.y;
        state
    }
}
