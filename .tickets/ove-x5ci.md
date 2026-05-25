---
id: ove-x5ci
status: open
deps: []
links: []
created: 2026-05-25T14:13:49Z
type: task
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, properties, style, accessibility]
---
# Clarify properties, pseudos, and accessibility semantics

`Overstory` needs a clear rule for what belongs as a dependency property, what belongs as a pseudoclass, what belongs as a template part, and what should eventually become accessibility metadata.

## Design

Fence: dependency properties own typed semantic data and invalidation; pseudoclasses own dynamic selector state; template parts own visual structure; accessibility metadata owns platform-facing semantics. None of these should impersonate the others.

Use web-like and WPF/WinUI-like comparisons carefully: placeholder might be a pseudo-element eventually, checked/selected/disabled are semantic state plus selector state, and accessibility roles/actions should not be inferred from visual styling alone.

## Acceptance Criteria

- Document criteria for adding a built-in property.
- Document criteria for adding a built-in pseudoclass.
- Audit current placeholder, selected, checked, read-only, focus-visible, and disabled states against those criteria.
- Identify the first accessibility metadata types needed by controls.
- Tests confirm controls keep semantic properties and pseudoclasses in sync.
- Source roadmaps reference this ticket only where a short comment would be ambiguous.
