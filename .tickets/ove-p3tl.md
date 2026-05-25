---
id: ove-p3tl
status: open
deps: []
links: []
created: 2026-05-25T14:12:24Z
type: epic
priority: 1
assignee:
tags: [roadmap, architecture]
---
# Roadmap: mature Overstory core systems

Track the detailed design memory behind the terse source-level roadmaps.

The source comments should orient readers; these tickets should carry rationale, constraints, examples, and acceptance criteria for implementation slices.

## Design

Use generated `tk` IDs as the durable references. Keep source roadmaps short, and add source references only where the inline sentence is otherwise too compressed to preserve intent.

Prefer child tickets that exercise shared systems rather than one-off demos: events, focus, style, templates, layout, presentation, composition, text, diagnostics, and host boundaries.

## Acceptance Criteria

- Child tickets exist for the first substantial roadmap areas.
- Each child ticket states the owning subsystem boundary.
- Each child ticket has concrete acceptance criteria that can drive implementation.
- Tickets remain architecture memory, not a replacement for local `rustdoc`.
