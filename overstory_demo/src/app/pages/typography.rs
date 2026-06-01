use super::super::*;
use super::metrics::append_large_counter_to;

#[derive(Clone, Copy, Debug)]
pub(super) struct TypographyPage;

impl View for TypographyPage {
    type Output = ();

    fn build(self, stage: &mut ChildUi<'_>) -> Self::Output {
        stage.with_child(
            Panel::new().layout(Layout::row(18.0).wrap().fill_width()),
            |row| {
                row.demo_card("TABULAR NUMBERS", PAGE_CARD_WIDTH, 326.0, |counter| {
                    append_large_counter_to(
                        counter,
                        "Steady suffix",
                        "events / min",
                        85_000_000,
                        257,
                    );
                    counter.add(
                        body_text("The number text uses the OpenType tabular-figures feature, so its measured width stays steady as digits change."),
                    );
                });

                row.demo_card("TEXT SURFACES", PAGE_CARD_WIDTH, 326.0, |text| {
                    text.add(
                        Text::new("Overstory keeps simple labels cheap while still resolving inherited text style through templates and generated parts.")
                            .font_size(16.0)
                            .class(CLASS_DEMO_BODY)
                            .wrap(),
                    );
                    text.add(Divider::horizontal().length(PAGE_CONTROL_WIDTH));
                    text.add(
                        TextInput::new()
                            .text("PlainEditor-backed editing")
                            .width(PAGE_CONTROL_WIDTH),
                    );
                    text.add(
                        TextInput::new()
                            .placeholder("IME, clipboard, accessibility next")
                            .width(PAGE_CONTROL_WIDTH),
                    );
                });
            },
        );
    }
}
