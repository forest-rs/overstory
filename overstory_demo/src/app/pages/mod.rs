mod anchoring;
mod box_model;
mod disclosure;
mod inputs;
mod layout;
mod metrics;
mod motion;
mod outline;
mod overview;
mod selection;
pub(super) mod svg;
mod systems;
mod transcript;
mod typography;
mod workbench;

use super::*;

fn clear_children(ui: &mut Ui, parent: ElementId) {
    let children: Vec<ElementId> = ui
        .get(parent)
        .map(|element| element.children().to_vec())
        .unwrap_or_default();
    for child in children {
        if ui.get(child).is_some() {
            ui.remove(child);
        }
    }
}

/// Test-only entry that renders a page without the live shell. Builds a
/// throwaway shell so the styling workbench still functions in isolation.
#[cfg(test)]
pub(super) fn show_demo_page(ui: &mut Ui, stage: ElementId, page: DemoPage) {
    let resources = ui.style_tokens::<DemoStyleTokens>().resources;
    let shell = Rc::new(DemoShellState::new(resources));
    show_demo_page_themed(ui, stage, page, &shell);
}

/// Renders a page into the stage, threading the live shell so the styling
/// workbench can persist accent edits and re-apply the theme.
pub(super) fn show_demo_page_themed(
    ui: &mut Ui,
    stage: ElementId,
    page: DemoPage,
    shell: &Rc<DemoShellState>,
) {
    clear_children(ui, stage);
    ui.with_children(stage, |stage| {
        stage.view(PageHeader(page));
        match page {
            DemoPage::Overview => stage.view(overview::OverviewPage),
            DemoPage::Workbench => stage.view(workbench::WorkbenchPage::new(Rc::clone(shell))),
            DemoPage::Inputs => stage.view(inputs::InputsPage),
            DemoPage::Disclosure => stage.view(disclosure::DisclosurePage),
            DemoPage::Systems => stage.view(systems::SystemsPage),
            DemoPage::Selection => stage.view(selection::SelectionPage),
            DemoPage::Outline => stage.view(outline::OutlinePage),
            DemoPage::Layout => stage.view(layout::LayoutPage),
            DemoPage::Anchoring => stage.view(anchoring::AnchoringPage),
            DemoPage::BoxModel => stage.view(box_model::BoxModelPage),
            DemoPage::Transcript => stage.view(transcript::TranscriptPage),
            DemoPage::Svg => stage.view(svg::SvgPage),
            DemoPage::Typography => stage.view(typography::TypographyPage),
            DemoPage::Motion => stage.view(motion::MotionPage),
        }
    });
}

#[derive(Clone, Copy, Debug)]
struct PageHeader(DemoPage);

impl View for PageHeader {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        let page = self.0;
        stage.with_child(
            Panel::row(18.0)
                .class(CLASS_DEMO_HEADER)
                .padding_all(18.0)
                .layout(
                    Layout::row(18.0).size(
                        LayoutSize::new()
                            .width(LayoutDimension::Percent(1.0))
                            .height(LayoutDimension::Logical(PAGE_HEADER_HEIGHT)),
                    ),
                ),
            |header| {
                header.add(
                    Panel::new()
                        .class(CLASS_DEMO_HEADER_ACCENT)
                        .layout(Layout::new().logical_size(5.0, 56.0).flex_shrink(0.0)),
                );
                header.with_child(
                    Panel::column(8.0).layout(
                        Layout::column(8.0)
                            .size(LayoutSize::new().min_width(LayoutDimension::Logical(0.0)))
                            .flex_grow(1.0)
                            .flex_shrink(1.0),
                    ),
                    |copy| {
                        copy.add(page_heading(page.title()));
                        copy.add(caption(page.subtitle()).wrap());
                    },
                );
            },
        );
    }
}
