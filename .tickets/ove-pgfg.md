---
id: ove-pgfg
status: open
deps: []
links: []
created: 2026-05-25T14:14:03Z
type: feature
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, controls, selection, overlay]
---
# Implement next controls that exercise shared systems

The control roadmap calls out near-term controls that are valuable because they force shared systems to mature: `RadioButton`/`RadioGroup`, `Select`/`ComboBox`, menu/flyout family, `TabView`/`TabControl`, `TextArea`, and hierarchical lists.

## Design

Fence: controls should compose existing Overstory systems instead of building private replacements. New controls explicitly should strengthen templates, routed events, focus, selection, scrolling, accessibility, presentation, and composition.

Do not add controls only as visual demos. Pick slices that prove shared architecture: `RadioGroup` for checked-state and grouping, `Select` for popup/list/selection/focus, `Menu` for keyboard routing and dismiss behavior, `TextArea` for editable prepared text and scrolling.

## Acceptance Criteria

- `RadioButton`/`RadioGroup` uses shared checked or selection state rather than bespoke booleans.
- `Select`/`ComboBox` composes `Button`, `Popup`, `ListBox`, `ScrollView`, and `understory_selection`.
- `Menu`/`Flyout` controls reuse overlay and routed keyboard behavior.
- `TextArea` waits for or drives the prepared editable-text rendering path.
- Each new control has focused tests for keyboard, focus, disabled state, styling, and template parts.
- New control APIs remain calm and do not expose lower-crate implementation details.
