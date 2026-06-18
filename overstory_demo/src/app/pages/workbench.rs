// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Styling & Theming workbench.
//!
//! Unlike the other gallery pages, this one is interactive: it drives a single
//! live control through the real systems. Every inspector edit calls the same
//! `set_local` / `clear_local` / `set_theme` paths an application would, so the
//! stage reacts through the genuine property, cascade, transition, and theme
//! machinery rather than a bespoke preview. It is also a forcing function for
//! the styling story; see [`append_provenance_note`] for the gap it exposes.

use overstory::pseudos::PSEUDO_HOVER;

use super::super::*;

const STAGE_WIDTH: f32 = 404.0;
const INSPECTOR_WIDTH: f32 = 372.0;
const HERO_FONT_SIZE: f64 = 16.0;
const INITIAL_CORNER_RADIUS: f64 = 10.0;
const INITIAL_BORDER_WIDTH: f64 = 1.0;

/// Shared closure that writes one editable dimension onto the subject.
type ApplyFn = Rc<dyn Fn(&mut Ui, ElementId, f64)>;

#[derive(Clone)]
pub(super) struct WorkbenchPage {
    shell: Rc<DemoShellState>,
}

impl WorkbenchPage {
    pub(super) fn new(shell: Rc<DemoShellState>) -> Self {
        Self { shell }
    }
}

impl View for WorkbenchPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        let shell = self.shell;
        stage.with_child(
            Panel::new().layout(
                Layout::row(24.0)
                    .wrap()
                    .fill_width()
                    .alignment(LayoutAlignment::new().align_items(LayoutAlign::Start)),
            ),
            |row| {
                let subject = build_stage(row);
                build_inspector(row, subject, Rc::clone(&shell));
            },
        );
    }
}

/// Builds the live stage: the hero subject plus a row of state specimens.
/// Returns the subject's `ElementId` so the inspector can drive it.
fn build_stage(row: &mut ChildUi<'_>) -> ElementId {
    row.with_child(
        Panel::column(20.0)
            .class(CLASS_DEMO_STAGE)
            .padding_all(28.0)
            .layout(
                Layout::column(20.0)
                    .size(LayoutSize::new().width(LayoutDimension::Logical(STAGE_WIDTH)))
                    .flex_shrink(1.0)
                    .alignment(
                        LayoutAlignment::new()
                            .align_items(LayoutAlign::Center)
                            .justify_content(LayoutJustify::Center),
                    ),
            ),
        |stage| {
            stage.add(card_title("LIVE SUBJECT"));
            let subject = stage.add(
                Button::new("Deploy")
                    .font_size(HERO_FONT_SIZE)
                    .corner_radius(INITIAL_CORNER_RADIUS)
                    .border_width(INITIAL_BORDER_WIDTH)
                    .tooltip("Driven live by the inspector; styled by the accent token"),
            );
            stage.add(
                caption(
                    "Hover, focus, and press the subject; the accent token colors every state.",
                )
                .wrap(),
            );
            stage.add(Divider::horizontal().length(STAGE_WIDTH - 56.0));
            stage.add(card_title("STATES"));
            build_state_specimens(stage);
            subject
        },
    )
}

/// A row of buttons frozen in each interaction state so a re-skin visibly
/// recolors hover/pressed/focus together.
fn build_state_specimens(stage: &mut ChildUi<'_>) {
    stage.with_child(
        Panel::new().layout(Layout::row(10.0).wrap().align_center()),
        |states| {
            specimen(states, "Default", None);
            specimen(states, "Hover", Some(PSEUDO_HOVER));
            specimen(states, "Pressed", Some(PSEUDO_PRESSED));
            specimen(states, "Focus", Some(PSEUDO_FOCUS_VISIBLE));
            let disabled = specimen(states, "Disabled", None);
            states.ui_mut().set_enabled(disabled, false);
        },
    );
}

fn specimen(states: &mut ChildUi<'_>, label: &str, pseudo: Option<PseudoClassId>) -> ElementId {
    let id = states.add(Button::new(label).font_size(13.0));
    if let Some(pseudo) = pseudo {
        states.ui_mut().add_pseudo(id, pseudo);
    }
    id
}

/// Builds the inspector: the panel of controls that drive the subject and the
/// theme. Each section dogfoods one system end to end.
fn build_inspector(row: &mut ChildUi<'_>, subject: ElementId, shell: Rc<DemoShellState>) {
    row.with_child(
        Panel::column(18.0)
            .class(CLASS_DEMO_CARD)
            .padding_all(18.0)
            .layout(
                Layout::column(18.0)
                    .size(LayoutSize::new().width(LayoutDimension::Logical(INSPECTOR_WIDTH)))
                    .flex_shrink(0.0),
            ),
        |inspector| {
            inspector.add(card_title("INSPECTOR"));

            // Geometry: live property writes through set_local + transitions,
            // driven by real sliders reporting through on_value_changed.
            section(inspector, "GEOMETRY", |geometry| {
                slider_row(
                    geometry,
                    subject,
                    "Corner radius",
                    INITIAL_CORNER_RADIUS,
                    (0.0, 24.0),
                    1.0,
                    "px",
                    Rc::new(apply_corner_radius),
                );
                slider_row(
                    geometry,
                    subject,
                    "Border width",
                    INITIAL_BORDER_WIDTH,
                    (0.0, 8.0),
                    0.5,
                    "px",
                    Rc::new(apply_border_width),
                );
                slider_row(
                    geometry,
                    subject,
                    "Font size",
                    HERO_FONT_SIZE,
                    (11.0, 28.0),
                    1.0,
                    "pt",
                    Rc::new(apply_font_size),
                );
            });

            // Content: semantic owner content projected by the template.
            section(inspector, "CONTENT", |content| {
                content.add(
                    SegmentedControl::new()
                        .options([
                            ChoiceOption::new("Deploy"),
                            ChoiceOption::new("Save changes"),
                            ChoiceOption::new("Continue"),
                        ])
                        .selected_index(0)
                        .on_selection_changed(move |ui, event| {
                            if let Some(item) = event.selected_item {
                                let content_prop = ui.properties().content;
                                ui.set_local(subject, content_prop, item);
                            }
                        }),
                );
            });

            // Template: same behavior, replaceable visual structure. "Default"
            // clears the local override so the theme template re-expands.
            section(inspector, "TEMPLATE", |template| {
                template.add(
                    SegmentedControl::new()
                        .options([ChoiceOption::new("Default"), ChoiceOption::new("Framed")])
                        .selected_index(0)
                        .on_selection_changed(move |ui, event| {
                            let template_prop = ui.properties().control_template;
                            match event.selected_item.as_deref() {
                                Some("Framed") => {
                                    ui.set_local(
                                        subject,
                                        template_prop,
                                        Some(framed_button_template().into()),
                                    );
                                }
                                _ => {
                                    ui.clear_local(subject, template_prop);
                                }
                            }
                        }),
                );
                template.add(
                    caption(
                        "\"Default\" clears the local override back to the theme template; \
                         \"Framed\" wraps the content presenter in a themed double surface.",
                    )
                    .wrap(),
                );
            });

            // Theme: the headline. Rebuild the theme with a new accent token
            // and re-apply it; the whole gallery re-skins.
            section(inspector, "ACCENT TOKEN", |accent| {
                let initial = accent_index(shell.accent());
                let shell_for_accent = Rc::clone(&shell);
                accent.add(
                    SegmentedControl::new()
                        .options(DemoAccent::ALL.map(|a| ChoiceOption::new(a.label())))
                        .selected_index(initial)
                        .on_selection_changed(move |ui, event| {
                            if let Some(item) = event.selected_item
                                && let Some(accent) = accent_from_label(&item)
                            {
                                shell_for_accent.set_accent(accent);
                                apply_demo_theme(ui, &shell_for_accent);
                            }
                        }),
                );
                accent.add(
                    caption("Rebuilds the theme and re-applies it; the whole gallery re-skins.")
                        .wrap(),
                );
            });

            append_provenance_section(inspector, subject);
        },
    );
}

/// A titled inspector section.
fn section<R>(
    inspector: &mut ChildUi<'_>,
    title: &str,
    f: impl FnOnce(&mut ChildUi<'_>) -> R,
) -> R {
    inspector.with_child(Panel::column(10.0), |group| {
        group.add(card_title(title));
        f(group)
    })
}

/// A labeled slider row with a live numeric readout that drives one dimension
/// onto the subject through `set_local`.
fn slider_row(
    parent: &mut ChildUi<'_>,
    subject: ElementId,
    label: &str,
    initial: f64,
    range: (f64, f64),
    step: f64,
    unit: &'static str,
    apply: ApplyFn,
) {
    let apply_for_cb = Rc::clone(&apply);
    parent.with_child(
        Panel::column(6.0).layout(Layout::column(6.0).fill_width()),
        |group| {
            let readout = group.with_child(
                Panel::row(8.0).layout(Layout::row(8.0).fill_width().align_center()),
                |header| {
                    header.add(
                        control_label(label).layout(
                            Layout::new()
                                .size(LayoutSize::new().min_width(LayoutDimension::Logical(0.0)))
                                .flex_grow(1.0),
                        ),
                    );
                    header.add(number_text(&format_value(initial, unit)))
                },
            );
            group.add(
                Slider::new()
                    .range(range.0, range.1)
                    .step(step)
                    .value(initial)
                    .layout(Layout::new().fill_width())
                    .on_value_changed(move |ui, event| {
                        apply_for_cb(ui, subject, event.value);
                        let text_prop = ui.properties().text;
                        ui.set_local(readout, text_prop, format_value(event.value, unit));
                    }),
            );
        },
    );
    // Make the subject match the initial readout (overriding theme defaults).
    apply(parent.ui_mut(), subject, initial);
}

fn format_value(value: f64, unit: &str) -> String {
    format!("{value:.0}{unit}")
}

fn apply_corner_radius(ui: &mut Ui, subject: ElementId, radius: f64) {
    let radii = ui.properties().surface.corner_radii;
    let value = CornerRadius::circular(radius);
    ui.set_local(subject, radii.top_left, value);
    ui.set_local(subject, radii.top_right, value);
    ui.set_local(subject, radii.bottom_right, value);
    ui.set_local(subject, radii.bottom_left, value);
}

fn apply_border_width(ui: &mut Ui, subject: ElementId, width: f64) {
    let widths = ui.properties().surface.border_widths;
    ui.set_local(subject, widths.top, width);
    ui.set_local(subject, widths.right, width);
    ui.set_local(subject, widths.bottom, width);
    ui.set_local(subject, widths.left, width);
}

fn apply_font_size(ui: &mut Ui, subject: ElementId, size: f64) {
    let font_size = ui.properties().font_size;
    ui.set_local(subject, font_size, size);
}

/// The "Framed" preset: a nested double surface that wraps the button's
/// content presenter. Reuses the demo's existing template classes.
fn framed_button_template() -> TemplateSpec {
    TemplateSpec::from_nodes([TemplateNode::surface()
        .with_class(CLASS_DEMO_TEMPLATE_BUTTON_OUTER)
        .with_child(
            TemplateNode::surface()
                .with_class(CLASS_DEMO_TEMPLATE_BUTTON_INNER)
                .with_child(TemplateNode::content_presenter(SLOT_BUTTON_CONTENT)),
        )])
}

fn accent_index(accent: DemoAccent) -> usize {
    DemoAccent::ALL
        .iter()
        .position(|a| *a == accent)
        .unwrap_or(0)
}

fn accent_from_label(label: &str) -> Option<DemoAccent> {
    DemoAccent::ALL.into_iter().find(|a| a.label() == label)
}

/// Surfaces a real toolkit gap, honestly. The workbench would like to show
/// "which selector set this value?" for each resolved property. understory now
/// computes that provenance internally, but overstory has not yet surfaced a
/// public `Ui::property_provenance` to read it — the next step for this page.
/// Live provenance readout: shows where the subject's corner radius and
/// background resolve from, refreshed on a timer so it reflects the current
/// resolved state after slider edits and accent changes.
fn append_provenance_section(inspector: &mut ChildUi<'_>, subject: ElementId) {
    section(inspector, "PROVENANCE", |note| {
        let radius_readout = note.add(caption("Corner radius: \u{2026}").wrap());
        let background_readout = note.add(caption("Background: \u{2026}").wrap());
        note.add(
            caption(
                "Corner radius is a local override (the inspector drives it); the background \
                 resolves from the theme, and the readout names the winning origin and resource.",
            )
            .wrap(),
        );

        let ui = note.ui_mut();
        let _ = ui.schedule_timer_repeating(radius_readout, millis(450));
        ui.on_timer(radius_readout, move |ui, _event| {
            refresh_provenance(ui, subject, radius_readout, background_readout);
        });
        refresh_provenance(ui, subject, radius_readout, background_readout);
    });
}

fn refresh_provenance(
    ui: &mut Ui,
    subject: ElementId,
    radius_readout: ElementId,
    background_readout: ElementId,
) {
    let radii = ui.properties().surface.corner_radii;
    let background = ui.properties().surface.background;
    let radius_source = ui
        .describe_property_source(subject, radii.top_left)
        .unwrap_or_default();
    let background_source = ui
        .describe_property_source(subject, background)
        .unwrap_or_default();
    let text_prop = ui.properties().text;
    ui.set_local(
        radius_readout,
        text_prop,
        format!("Corner radius: {radius_source}"),
    );
    ui.set_local(
        background_readout,
        text_prop,
        format!("Background: {background_source}"),
    );
}

#[cfg(test)]
mod tests {
    use understory_property::DependencyObjectExt;

    use super::*;

    #[test]
    fn apply_corner_radius_writes_every_corner_locally() {
        let mut ui = Ui::new();
        let root = ui.root();
        let subject = ui.append(root, Button::new("Deploy"));

        apply_corner_radius(&mut ui, subject, 18.0);

        let radii = ui.properties().surface.corner_radii;
        let element = ui.get(subject).expect("subject should exist");
        let expected = CornerRadius::circular(18.0);
        for corner in [
            radii.top_left,
            radii.top_right,
            radii.bottom_right,
            radii.bottom_left,
        ] {
            assert_eq!(
                element.get_local_value(corner).copied(),
                Some(expected),
                "each corner should carry the slider-applied radius",
            );
        }
    }

    #[test]
    fn accent_reskin_rebuilds_the_active_theme_token() {
        let mut ui = Ui::new();
        let resources = ui.style_tokens::<DemoStyleTokens>().resources;
        let shell = Rc::new(DemoShellState::new(resources));

        shell.set_accent(DemoAccent::Emerald);
        apply_demo_theme(&mut ui, &shell);

        assert_eq!(
            ui.theme().get::<Option<Brush>>(theme::ACCENT_BG).cloned(),
            Some(Some(Brush::from(DemoAccent::Emerald.base()))),
            "selecting an accent should re-skin the live ACCENT_BG token",
        );
    }

    #[test]
    fn accent_label_round_trips() {
        for accent in DemoAccent::ALL {
            assert_eq!(accent_from_label(accent.label()), Some(accent));
            assert_eq!(DemoAccent::ALL[accent_index(accent)], accent);
        }
    }
}
