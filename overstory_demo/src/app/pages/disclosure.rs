use super::super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct DisclosurePage;

impl View for DisclosurePage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_responsive_card("EXPANDER", PAGE_CARD_WIDTH, |expanders| {
                    expanders.add(
                        caption(
                            "Each Expander owns one disclosure state and a typed content slot. \
                             Opening one does not affect the other.",
                        )
                        .wrap(),
                    );
                    expanders.content(Expander::new("First section").expanded(true), |section| {
                        section.add(CheckBox::new("Option A").checked(true));
                        section.add(CheckBox::new("Option B"));
                    });
                    expanders.content(Expander::new("Second section"), |section| {
                        section.add(CheckBox::new("Option C"));
                        section.add(CheckBox::new("Option D"));
                    });
                });

                row.demo_responsive_card("ACCORDION", PAGE_CARD_WIDTH, |card| {
                    card.add(
                        caption(
                            "Accordion coordinates several sections. This one is single-required, \
                             so exactly one section stays open at a time.",
                        )
                        .wrap(),
                    );
                    card.with_child(Accordion::new().single_required(), |accordion| {
                        accordion.section_with(
                            AccordionSection::new("First").expanded(true),
                            |section| {
                                section.add(CheckBox::new("Option A").checked(true));
                                section.add(CheckBox::new("Option B"));
                            },
                        );
                        accordion.section("Second", |section| {
                            section.add(CheckBox::new("Option C"));
                            section.add(CheckBox::new("Option D"));
                        });
                        accordion.section("Third", |section| {
                            section.add(CheckBox::new("Option E"));
                            section.add(CheckBox::new("Option F"));
                        });
                    });
                });
            },
        );
    }
}
