---
id: ove-jeb7
status: open
deps: []
links: []
created: 2026-05-25T14:13:40Z
type: feature
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, diagnostics, invalidation, performance]
---
# Add diagnostics for the frame pipeline

The project tenets say diagnostics are architecture. Overstory needs structured visibility into invalidation, layout, text shaping, presentation, composition, timers, and event routing before larger examples become difficult to reason about.

## Design

Fence: `Ui` exposes structured diagnostics snapshots and counters; subsystem runtimes own the data they can measure accurately. Diagnostics explicitly should not require a particular renderer or host.

Start with cheap counters and snapshots: dirty channels, restyled elements, layout passes, measured widgets, text cache stats, presentation dirty nodes, composition updated surfaces, active timers, overlay stack, and event routes. Wind-tunnel crates can consume these for repeatable scenarios.

## Acceptance Criteria

- A public or crate-visible diagnostics snapshot reports frame pipeline work by subsystem.
- Text cache, presentation dirty nodes, and composition surface updates are observable.
- Event dispatch can optionally report route, phase, target, and handled state.
- Timer diagnostics include pending count and next deadline.
- Wind-tunnel examples can print or assert diagnostics without depending on internals.
- Diagnostics are no-std-compatible for core data structures.
