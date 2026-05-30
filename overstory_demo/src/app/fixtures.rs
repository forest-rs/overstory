// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

pub(super) const TIGER_SVG: &[u8] = include_bytes!("../../assets/Ghostscript_Tiger.svg");
pub(super) const RENDERER_QUEUE_LEN: usize = 1_000;
pub(super) const INITIAL_RENDERER_SELECTION: usize = 2;
pub(super) const RENDERER_STAGES: [&str; 12] = [
    "Vello scene graph",
    "CPU fallback raster",
    "Text shaping pass",
    "Accessibility tree",
    "Hit testing index",
    "Presentation cache",
    "Frame diagnostics",
    "Layer compaction",
    "Glyph atlas upload",
    "Clip stack rebuild",
    "Surface invalidation",
    "GPU submission",
];
pub(super) const SCENE_GRAPH_SVG: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="54" viewBox="0 0 100 54">
  <path id="surface" d="M8 13h31v18H8z" fill="#4f6fd6"/>
  <path id="text" d="M58 7h28v15H58z" fill="#64b37d"/>
  <path id="edge" d="M39 22 C49 22 48 14 58 14" fill="none" stroke="#d8dbea" stroke-width="3" stroke-linecap="round"/>
  <path id="output" d="M57 32h34v15H57z" fill="#d18b47"/>
  <path id="edge2" d="M39 28 C50 34 47 40 57 40" fill="none" stroke="#d8dbea" stroke-width="3" stroke-linecap="round"/>
</svg>
"##;
pub(super) fn renderer_queue_items() -> impl Iterator<Item = String> {
    (0..RENDERER_QUEUE_LEN).map(renderer_queue_label)
}

pub(super) fn renderer_queue_label(index: usize) -> String {
    let stage = RENDERER_STAGES[index % RENDERER_STAGES.len()];
    let batch = (index / RENDERER_STAGES.len()) + 1;
    format!("{stage} {batch:03}")
}

pub(super) fn renderer_outline_nodes() -> Vec<OutlineViewNode> {
    vec![
        OutlineViewNode::new(String::from("Renderer"))
            .with_first_child(Some(1))
            .with_next_sibling(Some(5)),
        OutlineViewNode::new(String::from("Scene graph lowering")).with_next_sibling(Some(2)),
        OutlineViewNode::new(String::from("Text shaping")).with_next_sibling(Some(3)),
        OutlineViewNode::new(String::from("Layer composition")).with_next_sibling(Some(4)),
        OutlineViewNode::new(String::from("AccessKit tree")),
        OutlineViewNode::new(String::from("Interaction"))
            .with_first_child(Some(6))
            .with_next_sibling(Some(9)),
        OutlineViewNode::new(String::from("Focus routing")).with_next_sibling(Some(7)),
        OutlineViewNode::new(String::from("Selection model")).with_next_sibling(Some(8)),
        OutlineViewNode::new(String::from("Scroll target registry")),
        OutlineViewNode::new(String::from("Motion")).with_first_child(Some(10)),
        OutlineViewNode::new(String::from("Implicit transitions")).with_next_sibling(Some(11)),
        OutlineViewNode::new(String::from("Timeline cues")),
    ]
}
