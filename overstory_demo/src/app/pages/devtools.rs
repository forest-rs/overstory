use super::super::*;
use super::svg::append_svg_inspector_row_to;

#[derive(Clone, Copy, Debug)]
pub(super) struct DevtoolsPage;

impl View for DevtoolsPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        let (mut target, selected) = devtools_target();
        let mut devtools =
            LiveDevtools::capture(&target, Some(selected)).expect("devtools should attach");
        let session = devtools.session().clone();

        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                let inspector_elements = row.demo_card(
                    "INSPECTION SNAPSHOT",
                    DEVTOOLS_INSPECTOR_WIDTH,
                    DEVTOOLS_CARD_HEIGHT,
                    |inspector| {
                        let inspector_id = inspector.parent();
                        inspector
                            .ui_mut()
                            .add_class(inspector_id, CLASS_DEVTOOLS_INSPECTOR_CARD);
                        append_session_view(inspector.ui_mut(), inspector_id, session.clone())
                    },
                );
                apply_devtools_target_refresh(&mut target);
                let refresh = devtools
                    .refresh_from(&target)
                    .expect("target refresh should flow through the devtools session");
                let _ = refresh_session_view(row.ui_mut(), inspector_elements, session.clone());

                row.demo_card(
                    "PROTOCOL BOUNDARY",
                    DEVTOOLS_SUMMARY_WIDTH,
                    DEVTOOLS_CARD_HEIGHT,
                    |summary| {
                        let summary_id = summary.parent();
                        summary
                            .ui_mut()
                            .add_class(summary_id, CLASS_DEVTOOLS_SUMMARY_CARD);
                        append_devtools_summary(summary, &session, refresh);
                    },
                );
            },
        );
    }
}

fn append_devtools_summary(
    summary: &mut ChildUi<'_>,
    session: &DevtoolsSessionHandle,
    refresh: DevtoolsRefresh,
) {
    summary.add(page_heading("Live protocol"));
    summary.add(body_text(
        "The inspector attaches to a target, receives refresh stream rows, then lazily asks for tree, selection, details, and summaries through the session boundary.",
    ));
    summary.add(Divider::horizontal().length(220.0));

    let client = InspectionClient::new(session.clone());
    let tree = client
        .element_tree()
        .unwrap_or_else(|_| session.with(|session| session.snapshot().inspection_element_tree()));
    let summaries = client.summaries().unwrap_or_else(|_| {
        session.with(|session| {
            overstory_devtools::InspectionSummaries::from_snapshot(session.snapshot())
        })
    });
    append_svg_inspector_row_to(summary, "Elements", &format!("{}", tree.nodes.len()));
    append_svg_inspector_row_to(
        summary,
        "Layers",
        &format!("{}", summaries.composition.layers.len()),
    );
    append_svg_inspector_row_to(
        summary,
        "Surfaces",
        &format!("{}", summaries.composition.surfaces.len()),
    );
    append_svg_inspector_row_to(
        summary,
        "Changed layers",
        &format!(
            "{}",
            summaries.composition.changes.added + summaries.composition.changes.removed
        ),
    );
    append_svg_inspector_row_to(
        summary,
        "Refresh stream rows",
        &format!("{}", refresh.stream_item_count),
    );
    append_svg_inspector_row_to(
        summary,
        "Selected",
        &refresh
            .selected
            .map(|id| format!("{}:{}", id.slot(), id.generation()))
            .unwrap_or_else(|| String::from("none")),
    );
    append_svg_inspector_row_to(
        summary,
        "Spoor envelopes",
        &session.with(|session| format!("{}", session.spoor_summary().envelope_count)),
    );
    append_svg_inspector_row_to(
        summary,
        "Spoor rows",
        &session.with(|session| {
            let summary = session.spoor_summary();
            format!(
                "{} contexts / {} counters / {} events",
                summary.context_count, summary.counter_count, summary.event_count
            )
        }),
    );
    append_svg_inspector_row_to(
        summary,
        "Timeline",
        &format!(
            "{} tracks / {} cues",
            summaries.timeline.active_tracks, summaries.timeline.pending_cues
        ),
    );
    append_svg_inspector_row_to(
        summary,
        "Frame",
        &format!(
            "{} layout passes / {} painted",
            summaries.frame.layout_passes, summaries.frame.painted_elements
        ),
    );
    summary.add(Divider::horizontal().length(220.0));
    summary.add(body_text(
        "Next: feed frame phases, traces, cache pressure, and timeline tracks through the same stream surface.",
    ));
}

fn devtools_target() -> (Ui, ElementId) {
    let mut target = Ui::new();
    target.set_theme(theme::dark_theme_builder().build());
    let root = target.root();
    target.set_layout(
        root,
        Layout::column(12.0)
            .logical_size(DEVTOOLS_TARGET_WIDTH, DEVTOOLS_TARGET_HEIGHT)
            .padding_all(18.0),
    );

    let deploy = target.with_children(root, |root| {
        root.with_child(Panel::row(12.0).align_center(), |header| {
            header.add(ToggleSwitch::new().checked(true));
            header.add(Text::new("Snapshot target").font_size(18.0));
        });

        let deploy = root.with_child(Panel::row(12.0).align_center(), |controls| {
            let deploy = controls.add(Button::new("Deploy").font_size(14.0));
            controls.add(CheckBox::new("Collect traces").checked(true));
            controls.add(ProgressBar::new().value(0.68).size(190.0, 18.0));
            deploy
        });

        root.with_child(Panel::row(14.0), |content| {
            content.add(
                ListBox::new()
                    .items(renderer_queue_items().take(48))
                    .size(260.0, 190.0)
                    .selected_index(4)
                    .scroll_offset_y(54.0)
                    .with_vertical_scrollbar(),
            );
            content.with_child(
                Panel::column(8.0).layout(Layout::column(8.0).logical_size(260.0, 190.0)),
                |detail| {
                    detail.add(Text::new("Composition").font_size(15.0));
                    detail.add(Text::new("Layer tree, surfaces, and timeline counters").wrap());
                    detail.add(Slider::new().value(0.42));
                },
            );
        });
        deploy
    });

    target.schedule_timeline_cue(millis(800), 1);
    compute_devtools_target_layout(&mut target);

    (target, deploy)
}

fn apply_devtools_target_refresh(target: &mut Ui) {
    let root = target.root();
    target.with_children(root, |root| {
        root.with_child(Panel::row(10.0).align_center(), |row| {
            row.add(Spinner::new());
            row.add(Text::new("Live target refresh applied").font_size(13.0));
            row.add(Button::new("Inspect").font_size(13.0));
        });
    });
    target.schedule_timeline_cue(millis(1_200), 2);
    compute_devtools_target_layout(target);
}

fn compute_devtools_target_layout(target: &mut Ui) {
    target.compute_layout(Size::new(
        f64::from(DEVTOOLS_TARGET_WIDTH),
        f64::from(DEVTOOLS_TARGET_HEIGHT),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::pages::show_demo_page;

    #[test]
    fn devtools_page_renders_live_inspection_and_summary() {
        let mut ui = build_ui();
        let stage = stage_element(&ui);
        show_demo_page(&mut ui, stage, DemoPage::Devtools);
        ui.compute_layout(Size::new(
            f64::from(INITIAL_SIZE.0),
            f64::from(INITIAL_SIZE.1),
        ));

        let inspector = find_by_class(&ui, stage, CLASS_DEVTOOLS_INSPECTOR_CARD)
            .expect("Devtools page should contain an inspector card");
        let summary = find_by_class(&ui, stage, CLASS_DEVTOOLS_SUMMARY_CARD)
            .expect("Devtools page should contain a summary card");

        assert!(
            count_type(&ui, inspector, overstory::TYPE_OUTLINE_VIEW) >= 1,
            "devtools card should render the protocol tree as an OutlineView",
        );
        assert!(
            ui.effective_bounds(inspector).width() > ui.effective_bounds(summary).width(),
            "inspector should get more room than the summary",
        );
    }

    #[test]
    fn devtools_target_refresh_reaches_live_mirror() {
        let (mut target, selected) = devtools_target();
        let mut devtools =
            LiveDevtools::capture(&target, Some(selected)).expect("devtools should attach");
        let initial_nodes = devtools.mirror().tree().nodes.len();

        apply_devtools_target_refresh(&mut target);
        let refresh = devtools
            .refresh_from(&target)
            .expect("target refresh should update live mirror");

        assert_eq!(refresh.stream_item_count, 1);
        assert_eq!(refresh.selected, Some(selected));
        assert!(
            devtools.mirror().tree().nodes.len() > initial_nodes,
            "refresh should expose target elements added after attachment",
        );
    }

    fn stage_element(ui: &Ui) -> ElementId {
        let root_children = ui
            .get(ui.root())
            .expect("root should exist")
            .children()
            .to_vec();
        let [_, stage] = *root_children else {
            panic!("gallery root should contain nav and stage");
        };
        stage
    }

    fn find_by_class(ui: &Ui, element: ElementId, class: ClassId) -> Option<ElementId> {
        let current = ui.get(element)?;
        if current.classes().contains(&class) {
            return Some(element);
        }
        current
            .children()
            .iter()
            .find_map(|child| find_by_class(ui, *child, class))
    }

    fn count_type(ui: &Ui, element: ElementId, type_tag: understory_style::TypeTag) -> usize {
        let Some(current) = ui.get(element) else {
            return 0;
        };
        let current_count = usize::from(current.type_tag() == Some(type_tag));
        current_count
            + current
                .children()
                .iter()
                .map(|child| count_type(ui, *child, type_tag))
                .sum::<usize>()
    }
}
