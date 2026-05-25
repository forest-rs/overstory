---
id: ove-hfm1
status: open
deps: []
links: []
created: 2026-05-25T14:13:09Z
type: feature
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, overlay, focus, composition]
---
# Unify overlay placement, focus scopes, and composition layers

`Tooltip`, `Popup`, select menus, context menus, flyouts, and future teaching UI need the same overlay infrastructure rather than independent stacks or ad hoc positioning.

## Design

Fence: `OverlayRuntime` owns overlay declarations, active overlay lifecycle, placement inputs, and dismissal metadata. It explicitly does not own each overlay control's content semantics or host rendering.

Placement should handle anchors, fallback placement, collision, viewport padding, transforms, and nested overlay coordinates. Focus scopes and restore-focus belong with `FocusRuntime` and routed events. Ordering and host handoff belong with `CompositionRuntime` and Subduction overlay layers.

## Acceptance Criteria

- `Tooltip` behavior cannot leave stale active entries when hover/focus changes.
- `Popup` overlays stack predictably and dismiss by outside pointer/Escape according to policy.
- Focus restoration and focus scopes are specified for nested overlays.
- Overlay placement handles viewport collision and anchor movement.
- Overlay elements render through composition overlay layers in deterministic order.
- `Select`/`ComboBox`/menu controls can reuse the overlay service without custom lifecycle code.
