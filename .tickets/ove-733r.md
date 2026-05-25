---
id: ove-733r
status: open
deps: []
links: []
created: 2026-05-25T14:13:30Z
type: feature
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, templates, style, diagnostics]
---
# Define template contracts and diagnostics

Templates need enough flexibility for real controls: extra parts, nested surfaces, repeated keyed parts, optional visuals, and control-specific layout. They also need diagnostics so template mistakes are obvious.

## Design

Fence: templates own generated visual subtree structure; controls own behavior, semantic state, and synchronization with required parts. Templates explicitly do not create a second layout or event system.

Support compact data-driven templates and custom code templates. Make `TemplateSlot`s and parts the control/template contract. Add missing-slot, duplicate-key, binding mismatch, and churn diagnostics before custom templates become hard to debug. Revisit pseudo-elements as the style-side complement to template parts.

## Acceptance Criteria

- Controls can declare required and optional `TemplateSlot`s with clear diagnostics.
- Repeated keyed parts can update incrementally without full subtree rebuilds where needed.
- Template bindings share the normal property-binding runtime.
- Missing required parts fail loudly in tests or diagnostics rather than silently degrading.
- A custom template can add extra nested surfaces without losing control behavior.
- Style rules for owner-local parts remain predictable and documented.
