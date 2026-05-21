# Overstory Public-Readiness Milestone

## Goal

Make Overstory's control/template/style path feel like one deliberate system:
controls own behavior and state, templates own replaceable visual structure,
styles target owners and named parts, and presentation remains resolved output.

## Non-Goals

- Do not revisit the public Taffy exposure in this milestone.
- Do not add text input, IME, selection, or accessibility work yet.
- Do not add broad new widgets just to increase surface area.
- Do not move layout into presentation primitives.

## Steps

1. Introduce `TemplateContext` and retained `TemplateInstance` for built-in
   templates. All part construction and template bindings go through the
   context, and control code stops relying on child indices.
2. Unify control state and activation policy across Button, Toggle, and Slider:
   enabled/disabled, focus, pressed state, keyboard activation, pointer/drag
   behavior, and canonical setter paths.
3. Add a data-driven `TemplateSpec` and use it for Button first. The spec should
   express nested surfaces, named parts, classes, and owner-to-part bindings
   while still building the normal element/layout tree.
4. Add repeated template parts through Slider ticks/axis: stable item keys,
   major/minor classes, tick and label parts, styling through the same selector
   system, and layout through ordinary elements.

## Risks

- Template storage can become a second tree if it owns structure instead of
  indexing the element tree. Keep `Element` as the runtime tree.
- Data-driven templates can drift into a layout language. Keep layout as
  properties on normal elements.
- Repeated parts can make control code unstable if keyed identity is omitted.
  Require stable keys before adding tick/label repeaters.
- Disabled/focus/press state can diverge if controls bypass canonical setters.
  Route all built-in behavior through the shared methods.

## Done

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- Built-in controls use named parts, default styles, and canonical state paths.
- The demo uses the same canonical app path as tests and docs.
