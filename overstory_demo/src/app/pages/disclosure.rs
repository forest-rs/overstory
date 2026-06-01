use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct DisclosurePage;

impl View for DisclosurePage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_card("EXPANDER", PAGE_CARD_WIDTH, 326.0, |expanders| {
                    expanders.add(
                        caption("Each Expander owns one disclosure state and a typed content slot. Opening one does not affect the other.")
                            .wrap(),
                    );
                    expanders.content(
                        Expander::new("Advanced routing").expanded(true),
                        |advanced| {
                            advanced.add(CheckBox::new("Require reviewer sign-off").checked(true));
                            advanced.add(CheckBox::new("Pause on anomaly"));
                        },
                    );
                    expanders.content(Expander::new("Audit trail"), |advanced| {
                        advanced.add(CheckBox::new("Record detailed timing"));
                        advanced.add(CheckBox::new("Attach render snapshots"));
                    });
                });

                row.demo_card("ACCORDION", PAGE_CARD_WIDTH, 326.0, |card| {
                    card.add(
                        caption("Accordion coordinates several sections. This one is single-required, so one section remains open.")
                            .wrap(),
                    );
                    card.with_child(Accordion::new().single_required(), |accordion| {
                        accordion.section_with(
                            AccordionSection::new("Routing").expanded(true),
                            |section| {
                                section
                                    .add(CheckBox::new("Require reviewer sign-off").checked(true));
                                section.add(CheckBox::new("Pause on anomaly"));
                            },
                        );
                        accordion.section("Audit trail", |section| {
                            section.add(CheckBox::new("Record detailed timing"));
                            section.add(CheckBox::new("Attach render snapshots"));
                        });
                        accordion.section("Delivery window", |section| {
                            section.add(CheckBox::new("Prefer low-traffic regions"));
                            section.add(CheckBox::new("Hold for business hours"));
                        });
                    });
                });
            },
        );
    }
}
