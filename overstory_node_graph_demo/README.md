# Field station instrument case

Twelve instrument modules must fit inside a 300 x 240 mm transport case.
Each module is fixed at 44 x 32 mm. Change the column count, inter-module
spacing, and plate corner radius in the native parameter card. The inline plan,
footprint, case-clearance result, and plate specification follow those values.
The shaded area beneath the case-dimension label is the usable case interior.
Modules that exceed it remain visible outside the footprint and turn coral.

Try six columns and 24 mm spacing: the footprint is 384 x 88 mm, exceeding the
case width by 84 mm. Two columns at the same spacing make a 112 x 312 mm
footprint, which is 72 mm too tall. The module dimensions stay fixed; fitting
the preview to the screen does not hide a layout that fails the case check.
This is a planar packing study, with no allowance for depth, connectors, foam,
manufacturing tolerance, or mechanical certification.

```sh
cargo run -p overstory_node_graph_demo
```

The host evaluates this small model in Rust. Layout inputs read their actual
graph bindings; reconnect the two length sources to try using corner radius as
spacing. Connections must have matching units and replace the existing incoming
binding. No execution engine or scheduler is embedded in node_graph.

Drag node bodies to arrange the view. Drag the gold cable tie to guide the three
parameter connections through ordered, distinct lanes. **Toggle cable tie**
compares guided and ordinary routes. A tie never merges ports or changes the
study's values. Native sliders support pointer, keyboard and accessibility input.

## Integration contracts

- Node moves and connection commits deliver scoped document/projection change
  receipts to retained invalidation state. The inspector distinguishes geometry
  visits and actual router calls from visibility scans.
- Node content uses flow layout. The Overstory adapter measures slider-track
  and text-row centers after arrangement and publishes node-local port anchors
  through scoped projection edits. Row coordinates are not shared with the
  headless model. Slot references survive control-template replacement.
- **Roomier controls** changes parameter text size and row spacing in place.
  Sockets and cables follow the arranged controls, and the case preview uses
  the remaining measured space. Names focus their controls; values include
  units and align to a common edge. Channel colors remain distinct even when
  two connections have the same unit.
- Measurement settles through the existing bounded element-realization phase.
  Subpixel rounding during panning does not become repeated projection edits.
  Wrapped-label, node-width and zoom checks cover attachment alignment, stable
  controls/focus, and convergence without layout feedback.
- The shaded case footprint has its dimensions attached above the drawing.
  Instruments remain visible outside its boundary, and overhanging modules
  use the warning color. Clearance feedback lives outside the drawing.
- Tied cables have ordered entry and exit leads. Untied cables use separate
  channels too, so releasing the guide does not collapse their routes together.
- Node bodies, controls, ports, cable guides and wire elements retain identities
  during local edits. Subpixel size rounding cannot replace a pressed node.
  Zoom can recreate size-dependent node content; viewport changes rebuild grid
  decoration. Input tests cover node dragging, slider capture, tie dragging and
  cancellation through the real event dispatcher.
- Wires use native retained vector paths. `set_vector_path` updates drawing intent
  while retaining element identity, and invalidates paint only for changed intent.
- Grid and wire layers are passive hit-test subtrees. Overstory's geometry query
  now honors pickability, so the viewport owns background pans and decorative
  paths cannot intercept them. Wheel input over a parameter control stays out of
  the canvas zoom handler.
- Parameter edits update the host model and retained preview without changing
  graph topology or deriving geometry. Moving a tie invalidates only its members;
  rebindings update its member handles while preserving lane order.
- The native adapter consumes computed-result reports for changed/removed
  bounds, anchors and routes, visibility membership and the preview. It stores
  element identities rather than copies of route geometry. Initial display,
  resets and skipped reports use a snapshot baseline; normal local updates
  touch only reported entities. Camera motion transforms materialized geometry
  explicitly because result reports use world coordinates.
- The inspector separates derive work from retained view updates. Dragging the
  case-check node updates one node, one socket and one wire; dragging the tie
  updates three wires and no node/socket geometry. Parameter changes update
  readings and the packing preview with zero graph-geometry updates. An equal
  reroute visits the router but produces no retained wire update. Input tests
  assert these counts while also checking capture, focus and stable identities.
- Tie membership, guide orientation and multiple-guide editing remain host-side
  experiments. The `mine/rope-experiment` WIP in Understory is a prospective
  backend for individual cable curves and endpoint tangents. Its simulation
  should advance on explicit application time, independently of graph derivation.
  No rope WIP code or simulation is included here.

The purpose of this demo is to exercise these boundaries with a useful small
problem. It does not prescribe a shared visual-language editor or application
framework.

## License

Licensed under either Apache-2.0 or MIT, at your option. See the repository root
for license details.
