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

pub(super) fn show_demo_page(ui: &mut Ui, stage: ElementId, page: DemoPage) {
    clear_children(ui, stage);
    ui.with_children(stage, |stage| {
        stage.view(PageHeader(page));
        match page {
            DemoPage::Overview => stage.view(overview::OverviewPage),
            DemoPage::Inputs => stage.view(inputs::InputsPage),
            DemoPage::Disclosure => stage.view(disclosure::DisclosurePage),
            DemoPage::Systems => stage.view(systems::SystemsPage),
            DemoPage::Selection => stage.view(selection::SelectionPage),
            DemoPage::Outline => stage.view(outline::OutlinePage),
            DemoPage::Layout => stage.view(layout::LayoutPage),
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
            Panel::row(18.0).class(CLASS_DEMO_HEADER).layout(
                Layout::row(18.0)
                    .size(
                        LayoutSize::new()
                            .width(LayoutDimension::Percent(1.0))
                            .height(LayoutDimension::Logical(PAGE_HEADER_HEIGHT)),
                    )
                    .padding_all(18.0),
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
