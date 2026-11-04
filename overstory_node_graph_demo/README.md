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

The long-term goal is for this demo to become a visually rich stress case for
animation, retained layout, hit testing, compositor-backed layers, and graph
editing ergonomics.

## License

Licensed under either Apache-2.0 or MIT, at your option. See the repository root
for license details.
