// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_timeline_model::{
    LaneId, TimelineDoc, TimelineFlow, TimelineLane, TimelineMarker, TimelineSpan,
};

pub(super) fn seed_timeline_doc() -> TimelineDoc {
    let lanes = [
        TimelineLane::new("frame"),
        TimelineLane::new("ui thread"),
        TimelineLane::new("renderer"),
        TimelineLane::new("gpu queue"),
    ];
    let spans = [
        span("frame 328", 2_430_000.0, 2_610_000.0, 0),
        span("frame 329", 2_650_000.0, 2_820_000.0, 0),
        span("frame 330", 2_880_000.0, 3_160_000.0, 0),
        span("frame 331", 3_210_000.0, 3_360_000.0, 0),
        span("event dispatch", 2_520_000.0, 2_585_000.0, 1),
        span("style resolve", 2_605_000.0, 2_735_000.0, 1),
        span("layout", 2_760_000.0, 2_985_000.0, 1),
        span("templates", 2_810_000.0, 2_925_000.0, 1).with_depth(1),
        span("paint tree", 3_015_000.0, 3_205_000.0, 1),
        span("scene build", 2_950_000.0, 3_190_000.0, 2),
        span("glyph atlas", 3_020_000.0, 3_115_000.0, 2).with_depth(1),
        span("upload", 3_230_000.0, 3_395_000.0, 2),
        span("submit", 3_310_000.0, 3_430_000.0, 3),
        span("present", 3_430_000.0, 3_470_000.0, 3),
    ];
    let markers = [
        TimelineMarker::new("input", 2_520_000.0, LaneId::new(1)),
        TimelineMarker::new_global("vsync", 2_760_000.0),
        TimelineMarker::new("commit", 3_205_000.0, LaneId::new(1)),
        TimelineMarker::new("submit", 3_310_000.0, LaneId::new(3)),
        TimelineMarker::new_global("present", 3_470_000.0),
    ];
    let flows = [
        TimelineFlow::new(
            understory_timeline_model::SpanId::new(8),
            understory_timeline_model::SpanId::new(9),
        ),
        TimelineFlow::new(
            understory_timeline_model::SpanId::new(11),
            understory_timeline_model::SpanId::new(12),
        ),
    ];

    TimelineDoc::try_from_parts(lanes, spans, markers, flows)
        .expect("seeded Sightline timeline should be valid")
}

fn span(label: &'static str, start: f64, end: f64, lane: usize) -> TimelineSpan {
    TimelineSpan::new(label, start, end, LaneId::new(lane))
}
