---
id: ove-9tcf
status: open
deps: []
links: []
created: 2026-05-25T14:13:20Z
type: task
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, layout, api]
---
# Keep Taffy behind Overstory layout intent

Overstory should expose its own layout vocabulary instead of making application code or controls traffic in raw Taffy types. The current `Layout` wrapper is the right direction, but it needs to grow deliberately as controls require more layout concepts.

## Design

Fence: `Layout` and `LayoutRuntime` own the boundary between Overstory layout intent and the current layout engine. They explicitly do not promise that Taffy is the permanent public model.

Add API only for concepts Overstory wants to support long term: min/max constraints, margin, flex basis, grid, overflow, axis-aware sizing, and bring-into-view. Prefer small builders and value types over mirroring every Taffy enum.

## Acceptance Criteria

- Public control APIs do not expose Taffy node ids or raw Taffy style types.
- Needed layout concepts for scroll, splitter, list, text, overlay, and viewport examples are represented by Overstory types.
- `LayoutRuntime` remains the only code that needs direct Taffy tree ownership.
- Tests cover at least one newly added layout concept through public Overstory API.
- Documentation states which layout concepts are intentionally not exposed yet.
