---
id: ove-b3av
status: open
deps: []
links: []
created: 2026-05-25T14:13:01Z
type: feature
priority: 2
assignee:
parent: ove-p3tl
tags: [roadmap, events, focus, input]
---
# Make routed events the first-class input system

Controls should not rely on demo or host glue for focus, pointer capture, keyboard activation, text input, tooltip behavior, or popup dismissal.

Routed events should be the shared mechanism for normalized input and control behavior.

## Design

Fence: `EventRuntime` owns routed handler storage, handler lanes, handled state, pointer capture, hover/click/drag state, and normalized dispatch over `ElementId` routes. It explicitly does not own raw platform event translation.

Use `understory_responder` for route/phase semantics, `ui-events` for normalized payloads, and Overstory for control-specific default handlers and public registration APIs.

The system should support WPF/WinUI-style capture/target/bubble, handled-events-too, control-vs-user handler ordering, and focus-visible/input-modality state.

## Acceptance Criteria

- Pointer down/up/move/enter/leave, scroll, key, focus, and text-input event families have a coherent registration story.
- Public capture/release APIs are documented and tested.
- Handled-events-too behavior is tested across user and control handlers.
- Focus interaction lives in `Ui`, not in host demos.
- `Tooltip`, `Popup`, `Slider`, `Splitter`, text input, and list behavior use routed event paths consistently.
- Event tracing or diagnostics can explain the route and handled state for a dispatch.
