// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use overstory::{
    Button, ChildUi, ElementId, ElementSpecExt, Layout, LayoutAlign, LayoutAlignment, Panel,
    SurfaceSpecExt, Text,
};

use crate::style::SightlineTokens;

use super::{
    BAR_COUNT, PLOT_HEIGHT, PLOT_WIDTH, TICK_COUNT, TIMELINE_LANE_COUNT, TIMELINE_MARKER_COUNT,
    TIMELINE_SPAN_COUNT,
};

#[derive(Clone, Debug)]
pub(crate) struct SampleProfilerView {
    pub(super) plot: ElementId,
    pub(super) summary_label: ElementId,
    pub(super) range_label: ElementId,
    pub(super) selection_label: ElementId,
    pub(super) zoom_in: ElementId,
    pub(super) zoom_out: ElementId,
    pub(super) fit: ElementId,
    pub(super) bars: Vec<ElementId>,
    pub(super) ticks: Vec<PlotTick>,
    pub(super) lanes: Vec<TimelineLaneView>,
    pub(super) spans: Vec<TimelineSpanView>,
    pub(super) markers: Vec<TimelineMarkerView>,
    pub(super) overview_window: ElementId,
    pub(super) selection_marker: ElementId,
    pub(super) callout: SampleCallout,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PlotTick {
    pub(super) line: ElementId,
    pub(super) label: ElementId,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TimelineLaneView {
    pub(super) label: ElementId,
    pub(super) rule: ElementId,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TimelineSpanView {
    pub(super) panel: ElementId,
    pub(super) label: ElementId,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TimelineMarkerView {
    pub(super) line: ElementId,
    pub(super) label: ElementId,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SampleCallout {
    pub(super) panel: ElementId,
    pub(super) title: ElementId,
    pub(super) time: ElementId,
    pub(super) duration: ElementId,
    pub(super) peak: ElementId,
}

#[derive(Clone, Debug)]
struct PlotViewParts {
    bars: Vec<ElementId>,
    ticks: Vec<PlotTick>,
    lanes: Vec<TimelineLaneView>,
    spans: Vec<TimelineSpanView>,
    markers: Vec<TimelineMarkerView>,
    overview_window: ElementId,
    selection_marker: ElementId,
    callout: SampleCallout,
}

pub(super) fn build(parent: &mut ChildUi<'_>, tokens: SightlineTokens) -> SampleProfilerView {
    parent
        .children(
            Panel::column(16.0)
                .class(tokens.instrument)
                .layout(Layout::column(16.0).fill_size())
                .padding(22.0)
                .border_width(1.0)
                .corner_radius(8.0),
            |surface| {
                let (summary_label, range_label, selection_label, zoom_in, zoom_out, fit) =
                    build_header(surface, tokens);
                let (plot, plot_parts) = build_plot(surface, tokens);
                build_status(surface, tokens);
                SampleProfilerView {
                    plot,
                    summary_label,
                    range_label,
                    selection_label,
                    zoom_in,
                    zoom_out,
                    fit,
                    bars: plot_parts.bars,
                    ticks: plot_parts.ticks,
                    lanes: plot_parts.lanes,
                    spans: plot_parts.spans,
                    markers: plot_parts.markers,
                    overview_window: plot_parts.overview_window,
                    selection_marker: plot_parts.selection_marker,
                    callout: plot_parts.callout,
                }
            },
        )
        .1
}

fn build_header(
    parent: &mut ChildUi<'_>,
    tokens: SightlineTokens,
) -> (
    ElementId,
    ElementId,
    ElementId,
    ElementId,
    ElementId,
    ElementId,
) {
    parent
        .children(
            Panel::row(12.0)
                .layout(
                    Layout::row(12.0)
                        .fill_width()
                        .alignment(LayoutAlignment::new().align_items(LayoutAlign::Center)),
                )
                .align_center(),
            |header| {
                header.add(
                    Text::new("Sample Profiler")
                        .font_size(24.0)
                        .class(tokens.title_text)
                        .layout(Layout::new().flex_shrink(0.0)),
                );
                let summary_label = header.add(
                    Text::new("")
                        .font_size(13.0)
                        .class(tokens.activity_text)
                        .layout(Layout::new().flex_shrink(0.0)),
                );
                let range_label = header.add(
                    Text::new("")
                        .font_size(13.0)
                        .class(tokens.secondary_text)
                        .layout(Layout::new().flex_shrink(0.0)),
                );
                header.add(Panel::new().layout(Layout::new().fill_width()));
                let zoom_out = header.add(Button::new("-").font_size(13.0));
                let zoom_in = header.add(Button::new("+").font_size(13.0));
                let fit = header.add(Button::new("Fit").font_size(13.0));
                let selection_label = header.add(
                    Text::new("")
                        .font_size(13.0)
                        .class(tokens.warning_text)
                        .layout(Layout::new().flex_shrink(0.0)),
                );
                (
                    summary_label,
                    range_label,
                    selection_label,
                    zoom_in,
                    zoom_out,
                    fit,
                )
            },
        )
        .1
}

fn build_plot(parent: &mut ChildUi<'_>, tokens: SightlineTokens) -> (ElementId, PlotViewParts) {
    parent.children(
        Panel::new()
            .class(tokens.plot)
            .layout(
                Layout::new()
                    .relative()
                    .logical_size(PLOT_WIDTH, PLOT_HEIGHT)
                    .flex_shrink(0.0),
            )
            .border_width(1.0)
            .corner_radius(8.0),
        |plot| {
            let mut ticks = Vec::with_capacity(TICK_COUNT);
            for _ in 0..TICK_COUNT {
                let line = plot.add(
                    Panel::new()
                        .class(tokens.plot_tick)
                        .layout(Layout::new().absolute_at(0.0, 0.0).logical_size(1.0, 1.0)),
                );
                let label = plot.add(
                    Text::new("")
                        .font_size(11.0)
                        .class(tokens.muted_text)
                        .layout(Layout::new().absolute_at(0.0, 0.0).flex_shrink(0.0)),
                );
                ticks.push(PlotTick { line, label });
            }

            let sample_lane = plot.add(
                Text::new("overview")
                    .font_size(11.0)
                    .class(tokens.secondary_text)
                    .layout(Layout::new().absolute_at(10.0, 78.0).flex_shrink(0.0)),
            );
            let mut lanes = Vec::with_capacity(TIMELINE_LANE_COUNT + 1);
            lanes.push(TimelineLaneView {
                label: sample_lane,
                rule: plot.add(
                    Panel::new()
                        .class(tokens.plot_tick)
                        .layout(Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0)),
                ),
            });
            for _ in 0..TIMELINE_LANE_COUNT {
                let label = plot.add(
                    Text::new("")
                        .font_size(11.0)
                        .class(tokens.secondary_text)
                        .layout(Layout::new().absolute_at(0.0, 0.0).flex_shrink(0.0)),
                );
                let rule = plot.add(
                    Panel::new()
                        .class(tokens.plot_tick)
                        .layout(Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0)),
                );
                lanes.push(TimelineLaneView { label, rule });
            }

            let mut bars = Vec::with_capacity(BAR_COUNT);
            for _ in 0..BAR_COUNT {
                bars.push(
                    plot.add(
                        Panel::new()
                            .class(tokens.plot_bar)
                            .layout(
                                Layout::new()
                                    .absolute_at(0.0, PLOT_HEIGHT)
                                    .logical_size(0.0, 0.0),
                            )
                            .corner_radius(2.0),
                    ),
                );
            }

            let overview_window = plot.add(
                Panel::new()
                    .layout(Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0))
                    .corner_radius(3.0),
            );

            let mut spans = Vec::with_capacity(TIMELINE_SPAN_COUNT);
            for _ in 0..TIMELINE_SPAN_COUNT {
                let (_, span) = plot.children(
                    Panel::new()
                        .layout(
                            Layout::row(0.0)
                                .absolute_at(-512.0, -512.0)
                                .logical_size(0.0, 0.0),
                        )
                        .padding(4.0)
                        .corner_radius(5.0),
                    |span| {
                        let label = span.add(
                            Text::new("")
                                .font_size(10.0)
                                .class(tokens.title_text)
                                .layout(Layout::new().flex_shrink(0.0)),
                        );
                        TimelineSpanView {
                            panel: span.parent(),
                            label,
                        }
                    },
                );
                spans.push(span);
            }

            let mut markers = Vec::with_capacity(TIMELINE_MARKER_COUNT);
            for _ in 0..TIMELINE_MARKER_COUNT {
                let line = plot.add(
                    Panel::new()
                        .class(tokens.plot_selection)
                        .layout(Layout::new().absolute_at(0.0, 0.0).logical_size(0.0, 0.0)),
                );
                let label = plot.add(
                    Text::new("")
                        .font_size(10.0)
                        .class(tokens.warning_text)
                        .layout(Layout::new().absolute_at(0.0, 0.0).flex_shrink(0.0)),
                );
                markers.push(TimelineMarkerView { line, label });
            }

            let selection_marker = plot.add(
                Panel::new().class(tokens.plot_selection).layout(
                    Layout::new()
                        .absolute_at(0.0, 0.0)
                        .logical_size(0.0, PLOT_HEIGHT),
                ),
            );
            let callout = build_callout(plot, tokens);

            PlotViewParts {
                bars,
                ticks,
                lanes,
                spans,
                markers,
                overview_window,
                selection_marker,
                callout,
            }
        },
    )
}

fn build_callout(parent: &mut ChildUi<'_>, tokens: SightlineTokens) -> SampleCallout {
    parent
        .children(
            Panel::new()
                .class(tokens.metric)
                .layout(
                    Layout::column(4.0)
                        .absolute_at(-256.0, -256.0)
                        .logical_size(0.0, 0.0),
                )
                .padding(10.0)
                .border_width(1.0)
                .corner_radius(6.0),
            |callout| {
                let title = callout.add(
                    Text::new("")
                        .font_size(12.0)
                        .class(tokens.title_text)
                        .layout(Layout::new().flex_shrink(0.0)),
                );
                let time = callout.add(
                    Text::new("")
                        .font_size(11.0)
                        .class(tokens.secondary_text)
                        .layout(Layout::new().flex_shrink(0.0)),
                );
                let duration = callout.add(
                    Text::new("")
                        .font_size(11.0)
                        .class(tokens.activity_text)
                        .layout(Layout::new().flex_shrink(0.0)),
                );
                let peak = callout.add(
                    Text::new("")
                        .font_size(11.0)
                        .class(tokens.muted_text)
                        .layout(Layout::new().flex_shrink(0.0)),
                );
                SampleCallout {
                    panel: callout.parent(),
                    title,
                    time,
                    duration,
                    peak,
                }
            },
        )
        .1
}

fn build_status(parent: &mut ChildUi<'_>, tokens: SightlineTokens) {
    parent.with_child(
        Panel::row(18.0)
            .layout(
                Layout::row(18.0)
                    .fill_width()
                    .alignment(LayoutAlignment::new().align_items(LayoutAlign::Center)),
            )
            .align_center(),
        |status| {
            status.add(
                Text::new("Chronostore envelope")
                    .font_size(13.0)
                    .class(tokens.activity_text)
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            status.add(
                Text::new("Viewport1D zoom/selection")
                    .font_size(13.0)
                    .class(tokens.secondary_text)
                    .layout(Layout::new().flex_shrink(0.0)),
            );
            status.add(
                Text::new("sampled frame durations")
                    .font_size(13.0)
                    .class(tokens.muted_text)
                    .layout(Layout::new().flex_shrink(0.0)),
            );
        },
    );
}
