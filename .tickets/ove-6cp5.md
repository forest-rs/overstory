---
id: ove-6cp5
status: open
deps: []
links: []
created: 2026-05-25T14:12:50Z
type: feature
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, presentation, text, performance]
---
# Add prepared presentation resources for text and pictures

Overstory currently shapes text for measurement and presentation through a shared text engine cache, but editable text and future pictures/scenes need a more explicit prepared-resource path.

The goal is to share expensive preparation across layout and paint without adding direct renderer paint hooks to widgets.

## Design

Fence: widgets resolve semantic intent into presentation; presentation owns renderer-neutral drawing data and prepared resource references. It explicitly does not become a renderer or bypass layout/templates.

Prepared text should allow `TextInput` and future `TextArea` to use Parley `PlainEditor` layout state, while simple `Text` widgets can keep sharing shaped label layouts across controls. Picture-like primitives should be keyed so Overstory can reference `imaging` scenes or lighter path resources without making every primitive know about every backend type.

This may belong partly in `understory_presentation` if the abstraction is generally useful.

## Acceptance Criteria

- A keyed prepared-resource shape is designed for text and picture-like content.
- `TextInput` has a path to render from `PlainEditor` layout state without direct `Widget::paint`.
- Layout and paint can reuse prepared text where width/style/content are compatible.
- Repeated labels still share cached shaped layouts.
- Tests cover cache reuse and invalidation when content, style, or width changes.
- Wind-tunnel scenarios can measure cache hit rates and preparation cost.
