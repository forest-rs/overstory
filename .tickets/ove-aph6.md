---
id: ove-aph6
status: open
deps: []
links: []
created: 2026-05-25T14:12:40Z
type: feature
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, architecture, geometry, element]
---
# Support multiple geometry boxes per element

Some future features need one semantic `ElementId` to map to more than one physical box or fragment: multiline text, rich text runs, inline objects, focus/adornment geometry, compositor surfaces, and accessibility ranges.

The core `Element` should stay semantic and small; subsystem-specific fragments should live in side runtimes keyed by `ElementId`.

## Design

Fence: `Element` owns semantic identity, tree links, property storage, classes, pseudos, part identity, and optional widget behavior. It explicitly does not own every geometry, presentation, composition, text, or accessibility fragment.

`GeometryRuntime` already hints at the right shape with `ElementSideMap<SmallVec<[NodeId; 1]>>`. Keep the common case allocation-free, but let geometry, presentation, composition, text, and accessibility runtimes own their own fragment maps as needed.

Avoid adding boxes/fragments/layers/selection rects directly to `Element`. That would make `Element` a dumping ground and weaken subsystem ownership.

## Acceptance Criteria

- `GeometryRuntime` documents the one-element-to-many-box invariant.
- Removal cleans every geometry node and reverse lookup for an element.
- Hit testing can map any fragment node back to the owning `ElementId`.
- Focus/navigation behavior is specified for elements with multiple focusable or hit-testable fragments.
- At least one regression test or prototype covers an element with multiple box-tree nodes.
- `Element` itself remains free of subsystem-specific fragment collections.
