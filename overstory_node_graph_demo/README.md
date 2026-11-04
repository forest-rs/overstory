# overstory_node_graph_demo

`overstory_node_graph_demo` is a focused native showcase for an element-based
node graph built with Overstory, `understory_node_graph`, and
`understory_view2d`.

Run it with:

```sh
cargo run -p overstory_node_graph_demo
```

The demo currently exercises:

- graph data, validation, derived layout, and orthogonal edge routing from
  `understory_node_graph`;
- viewport pan and zoom behavior from `understory_view2d`;
- Overstory elements for nodes, ports, labels, hit targets, tooltips, and
  interactions;
- dragging nodes and previewing edge creation without a separate canvas-item
  object model.

Node moves and connection commits use Understory's scoped edit receipts. The
demo retains its invalidation state between derives, so ordinary edits can
update neighboring geometry instead of forcing a full rebuild. The inspector
reports actual visited nodes and router calls separately from visibility scans.

The Surface parameters card contains three native sliders and a twelve-surface
inline study. Columns, spacing and corner radius belong to the application model;
they do not mutate graph topology or run an execution engine. Pointer, keyboard
and accessibility edits use the same slider value path. Preview edits retain the
controls and report zero graph geometry work. The card's output sockets use
node-local anchors beside the parameter rows.

Node bodies, controls, ports and committed route segments retain their element
identities across local edits. Changed geometry updates their layout; unchanged
routes keep their elements. Grid reconstruction follows viewport changes. The
host still compares computed snapshots: this deliberately exposes the remaining
need for a derived-change API rather than hiding the scan behind a full UI rebuild.

The gold cable tie guides the three parameter connections through distinct,
ordered lanes. Drag the tie or use **Toggle cable tie** to compare guided and
ordinary routing. Moving or releasing it invalidates only its member routes;
it never merges ports or edits graph topology. New-connection previews use the
ordinary router until a connection is committed and explicitly assigned.
This is an application-owned, horizontal-guide experiment, not a general
obstacle router or a durable cable-authoring format.

## Integration findings

- Scoped edits and explicit node-local sockets work unchanged in this native
  host as well as the notebook. No new graph-core API was needed for this slice.
- The strongest next shared API candidate is a stream of changed computed
  bounds, anchors, routes and visibility membership. The retained adapter can
  then consume deltas rather than comparing every snapshot.
- Rich content remains a host concern: controls measure and edit the inline
  study; node_graph owns the enclosing bounds and connections. Zoom currently
  recreates size-dependent node content; ordinary moves retain it.
- Cable ties need ordered edge membership, guide geometry and member-route
  invalidation. These compose with the existing router and invalidation hooks.
  Multiple guides, guide orientation and editing membership need a second
  experiment before becoming a shared model.
- The `mine/rope-experiment` WIP in Understory offers a plausible optional
  backend for individual cable curves and endpoint tangents. Its retained
  simulation should advance on explicit application time, independently of
  graph derivation. Tie membership and lane policy are separate from rope
  geometry. No rope WIP code or simulation is included in this demo.

The long-term goal is for this demo to become a visually rich stress case for
animation, retained layout, hit testing, compositor-backed layers, and graph
editing ergonomics.

## License

Licensed under either Apache-2.0 or MIT, at your option. See the repository root
for license details.
