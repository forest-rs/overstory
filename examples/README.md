# Overstory Examples

Runnable examples centered on one Overstory capability. The getting-started
examples are small enough to read top-to-bottom and copy from; advanced
examples can be larger when the capability only makes sense as an integrated
vertical slice.

These are deliberately *not* the control gallery (`overstory_demo`) or a real
app (`apps/sightline`) — those are reference material, not learning paths.

## Running

```sh
cargo run --bin hello
cargo run --bin binding
cargo run --bin dock
```

| Example | Bin | Shows |
| --- | --- | --- |
| [`hello`](hello/) | `hello` | The minimal end-to-end path: build a `Ui`, run it through the `overstory_app` host facade, handle a routed `Button` activation, and push state back into the tree. |
| [`binding`](binding/) | `binding` | The reactive path: a slider and `LocalState` joined by a two-way binding, then fanned out to a progress bar and two `.map()`-transformed labels — no synchronization handlers. Read it after `hello` to contrast manual `set_local` with declarative binding. |
| [`dock`](dock/) | `dock` | A retained workbench backed by `overstory_docking`. Drop on a pane center to make tabs, on a tab strip to insert, or on an edge to split. The toolbar switches between outline and full-layout previews and exercises collapse, reopening, and zoom. |

## Conventions

- One example per directory: `examples/<name>/`.
- Examples are never published (`publish = false`), so package and binary names
  are **bare** (`hello`, not `overstory_hello`) — `cargo run --bin <name>` just
  works, and the name reads as the concept, not the crate.
- Prefer `overstory::prelude::*` and the `overstory_app` facade over reaching
  into host or internal crates. An example that needs a deep import is a hint
  the public surface has a gap worth closing.
- Keep getting-started examples short enough to read in one sitting. Keep
  advanced examples focused on one integrated capability rather than turning
  them into general-purpose applications.
