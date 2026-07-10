# Overstory Examples

Small, runnable, getting-started examples. Each one is the smallest complete
program that shows a single slice of Overstory, meant to be read top-to-bottom
and copied from.

These are deliberately *not* the control gallery (`overstory_demo`) or a real
app (`apps/sightline`) — those are reference material, not learning paths.

## Running

```sh
cargo run --bin hello
cargo run --bin binding
```

| Example | Bin | Shows |
| --- | --- | --- |
| [`hello`](hello/) | `hello` | The minimal end-to-end path: build a `Ui`, run it through the `overstory_app` host facade, handle a routed `Button` activation, and push state back into the tree. |
| [`binding`](binding/) | `binding` | The reactive path: a slider and `LocalState` joined by a two-way binding, then fanned out to a progress bar and two `.map()`-transformed labels — no synchronization handlers. Read it after `hello` to contrast manual `set_local` with declarative binding. |

## Conventions

- One example per directory: `examples/<name>/`.
- Examples are never published (`publish = false`), so package and binary names
  are **bare** (`hello`, not `overstory_hello`) — `cargo run --bin <name>` just
  works, and the name reads as the concept, not the crate.
- Prefer `overstory::prelude::*` and the `overstory_app` facade over reaching
  into host or internal crates. An example that needs a deep import is a hint
  the public surface has a gap worth closing.
- Keep each example short enough to read in one sitting. If it grows a second
  concern, that's a second example.
